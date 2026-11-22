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
    context: Context,
    source_block: u32,
) -> Result<Instruction, Failure> {
    let player = if *who == serde_json::json!({"player":"you"}) {
        PlayerExpr::you(context)
    } else {
        let id = target_selection(who).ok_or_else(|| {
            unsupported(
                "mill.who",
                "requires exact controller or target-player actor",
            )
        })?;
        PlayerExpr::target(bind_target(program, slots, id, TargetDomain::AnyPlayer)?)
    };
    Ok(Instruction::mill(
        player,
        Amount::Fixed {
            value: controller_library::fixed_count(amount)?,
        },
        source_block,
    ))
}
