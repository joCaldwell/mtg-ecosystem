//! "Transform" (CR 701.27).
use super::*;

/// CR 701.27a,f: "transform this [permanent]" in the permanent's own
/// activated or triggered ability. The control layer peels "you may"
/// (CR 608.2d). Transforming another object, a set, or from a spell rejects.
/// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
pub(super) fn lower(
    cx: &mut StepCx<'_>,
    what: &serde_json::Value,
    who: &PlayerSelector,
    optional: bool,
) -> Result<Instruction, Failure> {
    uncounted(cx)?;
    if !matches!(
        typed::<ast::PlayerRef>(who, "player")?,
        ast::PlayerRef::You(_)
    ) {
        return Err(step("transform", "player"));
    }
    let what = object(what)?;
    if !is_this_permanent(&what) || !matches!(cx.context, Context::Activated | Context::Triggered) {
        return Err(step("transform", "operand"));
    }
    if optional {
        return Err(step("transform", "optional"));
    }
    Ok(cx.act(Verb::Transform { what: Ref::Source }))
}
