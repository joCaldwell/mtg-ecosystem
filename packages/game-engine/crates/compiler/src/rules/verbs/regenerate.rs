//! CR 701.19: create a replacement effect on one exact permanent.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

pub(super) fn lower(cx: &mut StepCx<'_>, what: &serde_json::Value) -> Result<Instruction, Failure> {
    uncounted(cx)?;
    let what = object(what)?;
    let object = match &what {
        ast::ObjectRef::SelfObject(_) if cx.context != Context::Spell => kir::Ref::Source,
        ast::ObjectRef::This(this) if this.noun == "creature" && cx.context != Context::Spell => {
            kir::Ref::Source
        }
        ast::ObjectRef::Target(_) => {
            let slot = if is_creature_target(&what) {
                bind_creature_target(cx.program, cx.slots, &what)?
            } else {
                bind_permanent_target(cx.program, cx.slots, &what)?
            };
            kir::Ref::Target { slot }
        }
        _ => return Err(step("regenerate", "unsupported recipient")),
    };
    Ok(cx.act(Verb::Regenerate {
        what: SetExpr::One { object },
    }))
}
