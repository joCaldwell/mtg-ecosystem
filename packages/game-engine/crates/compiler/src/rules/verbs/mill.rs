//! "Mill" (CR 701.17).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

pub(super) fn lower(cx: &mut StepCx<'_>, mill: &ast::EffectMill) -> Result<Instruction, Failure> {
    let ast::EffectMill {
        optional,
        decided_by,
        starting_with,
        for_each,
        for_each_beyond,
        for_each_vote,
        duration,
        who,
        amount,
        repeat_while,
    } = mill;
    if starting_with.is_some()
        || for_each_beyond.is_some()
        || for_each_vote.is_some()
        || duration.is_some()
        || repeat_while.is_some()
    {
        return Err(step("mill", "qualified"));
    }
    check_optional_chooser("mill", optional.is_some(), who, decided_by.as_deref())?;
    let player = pilot_player(cx, who)?;
    let amount = match for_each {
        Some(filter) => pilot_each_filter(cx, filter, amount, Some(&player))?,
        None => pilot_amount(cx, amount)?,
    };
    let verb = Verb::Mill { player, amount };
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
