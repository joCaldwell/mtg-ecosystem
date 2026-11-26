//! Complete AST shapes for predefined token creation (CR 111.10).
use super::*;
use serde_json::Value;

fn keys(value: &Value, expected: &[&str]) -> bool {
    value
        .as_object()
        .is_some_and(|o| o.len() == expected.len() && expected.iter().all(|k| o.contains_key(*k)))
}
/// `{"effect":"create-token","count":N,"token":{"predefined":K}}`: the
/// controller creates N predefined tokens.
fn predefined_tokens(step: &Value) -> Option<(u32, PredefinedToken)> {
    if !keys(step, &["effect", "count", "token"])
        || step["effect"] != "create-token"
        || !keys(&step["count"], &["amount", "value"])
        || step["count"]["amount"] != "fixed"
        || !keys(&step["token"], &["predefined"])
    {
        return None;
    }
    let count = u32::try_from(step["count"]["value"].as_u64()?).ok()?;
    let kind = match step["token"]["predefined"].as_str()? {
        "Treasure" => PredefinedToken::Treasure,
        "Gold" => PredefinedToken::Gold,
        "Food" => PredefinedToken::Food,
        "Clue" => PredefinedToken::Clue,
        _ => return None,
    };
    (count > 0).then_some((count, kind))
}

pub(super) fn claims(step: &Value) -> bool {
    predefined_tokens(step).is_some()
}

pub(super) fn lower(
    step: &Value,
    context: Context,
    source_block: u32,
) -> Result<Instruction, Failure> {
    let (count, kind) =
        predefined_tokens(step).ok_or_else(|| unsupported("compose.step", "no step table row"))?;
    Ok(Instruction::create_tokens(
        context,
        count,
        kir::TokenSpec::Predefined { kind },
        source_block,
    ))
}
