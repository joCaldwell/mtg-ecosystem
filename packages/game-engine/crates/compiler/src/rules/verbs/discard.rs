//! "Discard" (CR 701.9, 101.4, 608.2b-d).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

pub(super) fn lower(
    cx: &mut StepCx<'_>,
    discard: &ast::EffectDiscard,
) -> Result<Instruction, Failure> {
    let ast::EffectDiscard {
        optional,
        decided_by,
        starting_with,
        for_each,
        for_each_beyond,
        for_each_vote,
        duration,
        who,
        what,
        random,
    } = discard;
    if starting_with.is_some()
        || for_each_beyond.is_some()
        || for_each_vote.is_some()
        || duration.is_some()
        || optional.is_some()
        || decided_by.is_some()
        || *random == Some(false)
    {
        return Err(step("discard", "qualified"));
    }
    let player = pilot_player(cx, who)?;
    let random = *random == Some(true);
    let card_filter = typed::<ast::ObjectFilter>(&serde_json::json!({"cls": "card"}), "object")?;
    let amount = match what.as_ref() {
        ast::CostDiscardWhat::Hand => None,
        ast::CostDiscardWhat::ObjectRef(object) => match object.as_ref() {
            ast::ObjectRef::Filter(cards)
                if *cards.filter == card_filter
                    && cards.up_to.is_none()
                    && cards.group_constraints.is_none()
                    && cards.random.is_none() =>
            {
                Some(match for_each {
                    Some(filter) => pilot_each_filter(cx, filter, &cards.count, Some(&player))?,
                    None => pilot_amount(cx, &cards.count)?,
                })
            }
            ast::ObjectRef::All(cards) if matches!(who.as_ref(), ast::PlayerRef::You(_)) => {
                let hand = typed::<ast::ObjectFilter>(
                    &serde_json::json!({"cls": "card", "zone": {"zone": "hand", "owner": "your"}}),
                    "object",
                )?;
                if *cards.filter != hand {
                    return Err(step("discard", "whole_hand_filter"));
                }
                None
            }
            _ => return Err(step("discard", "card_selection")),
        },
    };
    if amount.is_none() && (random || for_each.is_some()) {
        return Err(step("discard", "whole_hand_qualified"));
    }
    Ok(cx.act(Verb::Discard {
        player,
        amount,
        random,
    }))
}
