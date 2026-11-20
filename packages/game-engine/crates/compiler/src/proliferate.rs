//! CR 603.6a and 701.34a (2026-09-25).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

pub(super) fn lower(
    source: &Source,
    result: &ParseResult,
) -> Result<Option<TriggeredAbility>, Failure> {
    let Some(ast) = mtg_contracts::proliferate::landfall_ast(&source.oracle_text) else {
        return Ok(None);
    };
    if decode::<serde_json::Value>(result.abilities.get(), "abilities")? != ast {
        return Err(unsupported(
            "abilities.trigger",
            "requires complete controller land entry and Proliferate",
        ));
    }
    Ok(Some(mtg_contracts::proliferate::landfall()))
}
