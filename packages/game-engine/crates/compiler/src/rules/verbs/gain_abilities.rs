//! CR 611.2c, 613.1f and 613.7b: a resolved grant captures the affected
//! permanents and applies in layer 6 from the timestamp of its creation.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

pub(super) fn keyword(grant: &ast::GainedAbility) -> Result<KeywordAbility, Failure> {
    let grant = match grant {
        ast::GainedAbility::Keyword(keyword) => keyword,
        ast::GainedAbility::KeywordChoice(_)
        | ast::GainedAbility::AbilitiesOf(_)
        | ast::GainedAbility::ThisAbility(_)
        | ast::GainedAbility::Quoted(_) => {
            return Err(operand("granted_ability", variant(grant)));
        }
    };
    let ast::GainedAbilityKeyword {
        keyword,
        protection_attachment_exception: None,
        cost_definition: None,
    } = &**grant
    else {
        return Err(step("gain-abilities", "qualified_keyword"));
    };
    let ast::KeywordInstance {
        keyword,
        cost: None,
        cost_modifications: None,
        restriction: None,
        amount: None,
        bindings: None,
        variable_constraints: None,
        from: None,
        filter: None,
        materials: None,
        companion: None,
        paired_with: None,
        enchant_player: None,
        prototype: None,
        additional_costs: None,
        gift: None,
    } = &**keyword
    else {
        return Err(step("gain-abilities", "parameterized_keyword"));
    };
    serde::Deserialize::deserialize(
        serde::de::value::StrDeserializer::<serde::de::value::Error>::new(keyword),
    )
    .map_err(|_| operand("granted_keyword", keyword))
}

pub(super) fn lower(
    cx: &mut StepCx<'_>,
    what: &serde_json::Value,
    abilities: &[serde_json::Value],
    duration: &SourceDuration,
) -> Result<Instruction, Failure> {
    uncounted(cx)?;
    let until = match duration.duration.as_str() {
        "end-of-turn" | "this-turn" => kir::Until::EndOfTurn,
        "your-next-turn" => kir::Until::ControllerNextTurn,
        other => return Err(operand("granted_ability_duration", other)),
    };
    let grants: Vec<ast::GainedAbility> = typed(&abilities, "granted_ability")?;
    let keywords = grants.iter().map(keyword).collect::<Result<Vec<_>, _>>()?;
    if !mtg_contracts::supported_temporary_keywords(&keywords) {
        return Err(step("gain-abilities", "unsupported_keyword_list"));
    }
    let what = match typed::<ast::EffectGainAbilitiesWhat>(what, "grant_recipient")? {
        ast::EffectGainAbilitiesWhat::ObjectRef(what) => *what,
        ast::EffectGainAbilitiesWhat::Recipient(_) => {
            return Err(operand("grant_recipient", "step_result"));
        }
    };
    let what = if is_set(&what) {
        lower_board_set(&what, cx.context)?
    } else if is_this_creature(&what) {
        SetExpr::source()
    } else if let Some(slot) = continuous::followup_target(cx, &what) {
        if keywords == [KeywordAbility::Haste] && !continuous::targets_creature(cx, slot) {
            return Ok(cx.act(Verb::SetCharacteristics {
                what: SetExpr::target(slot),
                values: kir::CharacteristicSet {
                    keywords,
                    ..Default::default()
                },
                until,
            }));
        }
        SetExpr::target(slot)
    } else {
        SetExpr::target(bind_creature_target(cx.program, cx.slots, &what)?)
    };
    if until != kir::Until::EndOfTurn {
        return Ok(cx.act(Verb::SetCharacteristics {
            what,
            values: kir::CharacteristicSet {
                keywords,
                ..Default::default()
            },
            until,
        }));
    }
    Ok(cx.act(Verb::ModifyUntil {
        what,
        layer: kir::Layer::Ability,
        change: kir::Change::AddKeywords { keywords },
        until: kir::Until::EndOfTurn,
    }))
}
