//! Step table for instruction shapes outside the shared typed step mirror.
//!
//! Each row matches one complete step object - every key and operand - and
//! emits one existing runtime instruction. A step no row claims is decoded by
//! the shared typed mirror instead, so adding a row never changes how an
//! already supported step lowers.
//!
//! CR 508.1d, 509.1b: a creature that "can't block" or "can't be blocked"
//! this turn is a restriction that lasts until cleanup (CR 514.2).
//! CR 111.10: Treasure, Food and Clue are predefined tokens.
//! CR 701.6a, 115.1: a counter step announces one spell of the named type.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::Value;

fn keys(value: &Value, expected: &[&str]) -> bool {
    value
        .as_object()
        .is_some_and(|o| o.len() == expected.len() && expected.iter().all(|k| o.contains_key(*k)))
}

/// `{"ref":"target","filter":{"types":["creature"]},"count":1,"selectionId":S}`
fn one_target_creature(what: &Value) -> Option<&str> {
    (keys(what, &["ref", "filter", "count", "selectionId"])
        && what["ref"] == "target"
        && what["filter"] == serde_json::json!({"types":["creature"]})
        && what["count"] == serde_json::json!({"amount":"fixed","value":1}))
    .then(|| what["selectionId"].as_str())
    .flatten()
}

fn restriction(step: &Value) -> Option<(CreatureRestriction, &str)> {
    if !keys(step, &["effect", "action", "duration", "what"])
        || step["effect"] != "cant"
        || step["duration"] != serde_json::json!({"duration":"this-turn"})
    {
        return None;
    }
    let restriction = match step["action"].as_str()? {
        "block" => CreatureRestriction::CantBlock,
        "be-blocked" => CreatureRestriction::CantBeBlocked,
        _ => return None,
    };
    Some((restriction, one_target_creature(&step["what"])?))
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
        "Food" => PredefinedToken::Food,
        "Clue" => PredefinedToken::Clue,
        _ => return None,
    };
    (count > 0).then_some((count, kind))
}

/// `{"effect":"counter","what":{target spell of exactly one card type}}`.
fn typed_counter(step: &Value) -> Option<(CardKind, &str)> {
    let what = &step["what"];
    if !keys(step, &["effect", "what"])
        || step["effect"] != "counter"
        || !keys(what, &["ref", "filter", "count", "selectionId"])
        || what["ref"] != "target"
        || what["count"] != serde_json::json!({"amount":"fixed","value":1})
        || !keys(&what["filter"], &["cls", "types"])
        || what["filter"]["cls"] != "spell"
    {
        return None;
    }
    let [kind] = what["filter"]["types"].as_array()?.as_slice() else {
        return None;
    };
    Some((
        serde_json::from_value(kind.clone()).ok()?,
        what["selectionId"].as_str()?,
    ))
}

/// True when a table row owns this step's complete shape.
pub(super) fn claims(step: &Value) -> bool {
    restriction(step).is_some()
        || predefined_tokens(step).is_some()
        || typed_counter(step).is_some()
}

pub(super) fn lower(
    step: &Value,
    program: &mut SpellProgram,
    slots: &mut BTreeMap<String, u32>,
    source_block: u32,
) -> Result<Instruction, Failure> {
    if let Some((restriction, selection)) = restriction(step) {
        return Ok(Instruction::GrantCreatureRestriction {
            target_slot: bind_target(program, slots, selection, TargetDomain::BattlefieldCreature)?,
            restriction,
            source_block,
        });
    }
    if let Some((count, kind)) = predefined_tokens(step) {
        return Ok(Instruction::CreatePredefinedTokens {
            count,
            kind,
            source_block,
        });
    }
    if let Some((kind, selection)) = typed_counter(step) {
        let slot = bind_target(program, slots, selection, TargetDomain::StackSpell)?;
        let filter = PermanentFilter {
            types: vec![kind],
            non_types: vec![],
            non_colors: vec![],
            opponent: false,
            tapped: false,
            power: None,
        };
        let target = &mut program.targets[slot as usize];
        if target.filter.as_ref().is_some_and(|old| *old != filter) {
            return Err(unsupported("counter", "conflicting target filter"));
        }
        target.filter = Some(filter);
        return Ok(Instruction::CounterStackObject {
            target_slot: slot,
            source_block,
        });
    }
    Err(unsupported("compose.step", "no step table row"))
}
