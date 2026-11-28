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
    if optional.is_none() && decided_by.is_some() {
        return Err(step("draw", "qualified"));
    }
    let player = pilot_player(cx, who)?;
    let amount = match for_each {
        Some(filter) => pilot_each_filter(cx, filter, amount, Some(&player))?,
        None => pilot_amount_relative(cx, amount, Some(&player))?,
    };
    let chooser = match decided_by.as_deref().unwrap_or(who) {
        ast::PlayerRef::You(_) => PlayerExpr::you(cx.context),
        named if named == who.as_ref() => player.clone(),
        _ => return Err(unsupported("compose.control", "optional_chooser")),
    };
    let verb = Verb::Draw { player, amount };
    if optional.is_some() {
        Ok(Instruction::Kernel {
            node: kir::Node::Optional {
                chooser,
                node: Box::new(kir::Node::Do(verb)),
            },
            source_block: cx.source_block,
        })
    } else {
        Ok(cx.act(verb))
    }
}
