//! CR 115.3, 608.2b, 608.2c: one instruction can name several different
//! target occurrences. Homogeneous actions share one operand set and event.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

pub(super) fn lower(
    cx: &mut StepCx<'_>,
    members: &[serde_json::Value],
) -> Result<Instruction, Failure> {
    uncounted(cx)?;
    let unsupported_group = || step("simultaneous", "unlowered");
    if members.len() < 2
        || members.iter().any(|member| {
            !matches!(member["effect"].as_str(), Some("destroy" | "tap" | "untap"))
                || member["effect"] != members[0]["effect"]
                || member["what"]["ref"] != "target"
                || member["what"].get("upTo").is_some()
        })
    {
        return Err(unsupported_group());
    }
    let mut slots = Vec::with_capacity(members.len());
    for member in members {
        let member: Step = typed(member, "simultaneous")?;
        let Instruction::Kernel { node, .. } = lower_step(cx, &member)?;
        let slot = match node {
            kir::Node::Do(
                Verb::Destroy {
                    what:
                        SetExpr::One {
                            object: Ref::Target { slot },
                        },
                    regeneration_forbidden: false,
                }
                | Verb::Tap {
                    what:
                        SetExpr::One {
                            object: Ref::Target { slot },
                        },
                }
                | Verb::Untap {
                    what:
                        SetExpr::One {
                            object: Ref::Target { slot },
                        },
                },
            ) => slot,
            _ => return Err(unsupported_group()),
        };
        if slots.last().is_some_and(|prior| *prior >= slot) {
            return Err(unsupported_group());
        }
        slots.push(slot);
    }
    let what = SetExpr::Targets { slots };
    Ok(cx.act(match members[0]["effect"].as_str() {
        Some("destroy") => Verb::Destroy {
            what,
            regeneration_forbidden: false,
        },
        Some("tap") => Verb::Tap { what },
        Some("untap") => Verb::Untap { what },
        _ => unreachable!("member verb verified"),
    }))
}
