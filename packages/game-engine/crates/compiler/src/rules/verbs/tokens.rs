//! "Create tokens" (CR 111).
use super::*;

/// CR 111.1-111.4: "you" create a positive fixed number of tokens of one
/// exact creature recipe. Predefined tokens are step-table rows.
pub(super) fn lower(
    cx: &mut StepCx<'_>,
    who: Option<&serde_json::Value>,
    count: &Count,
    token: &SourceToken,
) -> Result<Instruction, Failure> {
    uncounted(cx)?;
    if who.is_some() {
        return Err(step("create-token", "controller"));
    }
    let count = fixed_positive(count)?
        .to_u32()
        .ok_or_else(|| operand("amount", "fixed:range"))?;
    Ok(Instruction::create_tokens(
        cx.context,
        count,
        kir::TokenSpec::Creature {
            spec: lower_creature_token(token)?,
        },
        cx.source_block,
    ))
}
