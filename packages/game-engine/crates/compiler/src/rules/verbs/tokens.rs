//! "Create tokens" (CR 111).
use super::*;

/// CR 111.1-111.4, 111.10, 608.2h: the resolving controller or the last
/// controller of a removed target creates tokens of one exact recipe: a
/// predefined artifact (Treasure, Gold, Food, Clue) or a creature token with
/// printed characteristics, entering tapped if printed. The number is fixed, the announced X, the
/// sentence's bound X or "for each [filter]", read when the step is
/// performed. Event results ("that many"), copies, printed names, abilities
/// and entry modifiers reject by name.
/// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
pub(super) fn lower(
    cx: &mut StepCx<'_>,
    who: Option<&serde_json::Value>,
    count: &serde_json::Value,
    token: &serde_json::Value,
    for_each: Option<&serde_json::Value>,
) -> Result<Instruction, Failure> {
    let (token, tapped) = recipe(token)?;
    let count = amount(cx, count, for_each)?;
    let controller = match who
        .map(|who| typed::<ast::PlayerRef>(who, "player"))
        .transpose()?
    {
        None | Some(ast::PlayerRef::You(_)) => PlayerExpr::you(cx.context),
        Some(ast::PlayerRef::Controller(reference)) => PlayerExpr::ControllerOf {
            object: kir::Ref::Target {
                slot: preceding_target(cx.program, cx.source_block, &reference.of)?,
            },
        },
        Some(other) => return Err(operand("token_controller", variant(&other))),
    };
    Ok(cx.act(Verb::CreateTokens {
        controller,
        count,
        token,
        tapped,
    }))
}

/// CR 111.10: the predefined artifact tokens with standard templates; CR
/// 111.3-111.4: a creature token with exactly its printed characteristics.
/// Returns whether the tokens enter tapped (CR 614.1c).
fn recipe(token: &serde_json::Value) -> Result<(kir::TokenSpec, bool), Failure> {
    let object = token.as_object().ok_or_else(|| operand("token", "shape"))?;
    if object.contains_key("copyOf") {
        return Err(operand("token", "copy_of"));
    }
    if !object.contains_key("predefined") {
        let spec: SourceToken = typed(token, "token")?;
        return Ok((
            kir::TokenSpec::Creature {
                spec: lower_creature_token(&spec)?,
            },
            spec.tapped == Some(true),
        ));
    }
    let ast::TokenSpecPredefined {
        predefined,
        tapped,
        attacking,
        attacking_defender,
        blocking,
    } = typed(token, "token")?;
    if attacking.is_some() || attacking_defender.is_some() || blocking.is_some() {
        return Err(operand("token", "attacking"));
    }
    let kind = match predefined {
        ast::PredefinedTokenName::Treasure => PredefinedToken::Treasure,
        ast::PredefinedTokenName::Gold => PredefinedToken::Gold,
        ast::PredefinedTokenName::Food => PredefinedToken::Food,
        ast::PredefinedTokenName::Clue => PredefinedToken::Clue,
        other => return Err(operand("token", format!("predefined:{}", variant(&other)))),
    };
    Ok((kir::TokenSpec::Predefined { kind }, tapped == Some(true)))
}

/// CR 107.3, 608.2h: how many tokens, as an amount the executor reads when
/// the step is performed.
fn amount(
    cx: &mut StepCx<'_>,
    count: &serde_json::Value,
    for_each: Option<&serde_json::Value>,
) -> Result<Amount, Failure> {
    let count = typed::<ast::Amount>(count, "amount")?;
    let bound = cx.bound_x();
    if let Some(for_each) = for_each {
        if bound.is_some() {
            return Err(operand("forEach", "per_object_amount"));
        }
        return lower_for_each(for_each, &count, cx.context);
    }
    match count {
        ast::Amount::X(_) => Ok(bound.unwrap_or_else(Amount::announced_x)),
        _ if bound.is_some() => Err(unsupported("bindings", "unconsumed count binding")),
        ast::Amount::Count(count) => lower_count(&count, cx.context),
        other => Ok(Amount::Fixed {
            value: fixed_positive_amount(&other)?,
        }),
    }
}
