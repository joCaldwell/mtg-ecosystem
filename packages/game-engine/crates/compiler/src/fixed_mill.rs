//! Whole ordinary fixed mill actors, CR109.5/115.1/701.17.
use super::*;
pub(super) fn target_selection(who: &serde_json::Value) -> Option<&str> {
    let object = who.as_object()?;
    if object.len() != 2 || who["player"] != "target-player" {
        return None;
    }
    who["selectionId"].as_str().filter(|s| !s.is_empty())
}
pub(super) fn instruction(
    program: &mut SpellProgram,
    slots: &mut BTreeMap<String, u32>,
    who: &serde_json::Value,
    amount: &serde_json::Value,
    source_block: u32,
) -> Result<Instruction, Failure> {
    let recipient = if *who == serde_json::json!({"player":"you"}) {
        None
    } else {
        let id = target_selection(who).ok_or_else(|| {
            unsupported(
                "mill.who",
                "requires exact controller or target-player actor",
            )
        })?;
        Some(LifeRecipient::Target {
            target_slot: bind_target(program, slots, id, TargetDomain::AnyPlayer)?,
        })
    };
    Ok(Instruction::MillCards {
        recipient,
        count: controller_library::fixed_count(amount)?,
        source_block,
    })
}
