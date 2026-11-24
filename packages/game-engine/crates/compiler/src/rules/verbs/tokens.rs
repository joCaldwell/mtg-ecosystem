//! "Create tokens" (CR 111).
use super::*;

/// CR 111.1-111.4, 608.2h: the resolving controller or the last controller
/// of a removed target creates tokens of one exact creature recipe.
/// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
pub(super) fn lower(
    cx: &mut StepCx<'_>,
    who: Option<&serde_json::Value>,
    count: &Count,
    token: &SourceToken,
) -> Result<Instruction, Failure> {
    uncounted(cx)?;
    let count = fixed_positive(count)?
        .to_u32()
        .ok_or_else(|| operand("amount", "fixed:range"))?;
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
        count: Amount::fixed(i64::from(count)),
        token: kir::TokenSpec::Creature {
            spec: lower_creature_token(token)?,
        },
    }))
}
