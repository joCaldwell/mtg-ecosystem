//! CR 602.5b-d, 302.6, 307.5 and 507.2 (2026-09-25). The printed
//! restriction is a permission gate, separate from the cost and effect.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

pub(super) fn lower(
    source: Option<Value>,
    line: &str,
) -> Result<(SpellTiming, Option<ActivationRestriction>), Failure> {
    let reject = || unsupported("restriction", "unsupported activation restriction");
    let Some(source) = source else {
        return if line.contains(". Activate only ") {
            Err(reject())
        } else {
            Ok((SpellTiming::Instant, None))
        };
    };
    let (timing, window, phrase) = if source == json!({"restriction":"once-each-turn"}) {
        (SpellTiming::Instant, None, "once each turn")
    } else {
        let primary = source
            .get("restrictions")
            .and_then(Value::as_array)
            .and_then(|terms| match terms.as_slice() {
                [primary, once] if *once == json!({"restriction":"once-each-turn"}) => {
                    Some(primary)
                }
                _ => None,
            })
            .unwrap_or(&source);
        if *primary == json!({"restriction":"sorcery"}) {
            (SpellTiming::MainPhase, None, "as a sorcery")
        } else {
            let (window, phrase) = [
                (
                    json!({"restriction":"timing","turn":"your"}),
                    ActivationWindow::YourTurn,
                    "during your turn",
                ),
                (
                    json!({"restriction":"timing","turn":"your","phase":"upkeep"}),
                    ActivationWindow::YourUpkeep,
                    "during your upkeep",
                ),
                (
                    json!({"restriction":"timing","turn":"your","before":"declare-attackers"}),
                    ActivationWindow::YourTurnBeforeAttackers,
                    "during your turn, before attackers are declared",
                ),
                (
                    json!({"restriction":"timing","phase":"combat"}),
                    ActivationWindow::Combat,
                    "during combat",
                ),
            ]
            .into_iter()
            .find_map(|(shape, window, phrase)| (*primary == shape).then_some((window, phrase)))
            .ok_or_else(reject)?;
            (SpellTiming::Instant, Some(window), phrase)
        }
    };
    let combined = source.get("restrictions").is_some();
    if combined && source.get("restriction") != Some(&json!("all-of"))
        || combined && source.as_object().is_none_or(|o| o.len() != 2)
    {
        return Err(reject());
    }
    let extra = if combined { " and only once each turn" } else { "" };
    if !line.ends_with(&format!("Activate only {phrase}{extra}.")) {
        return Err(reject());
    }
    let once_each_turn = combined || phrase == "once each turn";
    let restriction =
        (window.is_some() || once_each_turn).then_some(ActivationRestriction { window, once_each_turn });
    Ok((timing, restriction))
}
