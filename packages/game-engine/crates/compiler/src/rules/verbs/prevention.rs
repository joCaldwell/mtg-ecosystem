//! CR 615.1, 615.7, 611.2a: prevention effects created by a resolving spell
//! or ability, lasting this turn.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use kir::{DamageKindFilter, PreventionShield, PreventionSources, ResolvedPrevention};

/// The archived damage kind of a prevention or damage event.
pub(crate) fn damage_kind(kind: ast::ReplacementEventDamageKind) -> DamageKindFilter {
    match kind {
        ast::ReplacementEventDamageKind::All => DamageKindFilter::All,
        ast::ReplacementEventDamageKind::Combat => DamageKindFilter::Combat,
        ast::ReplacementEventDamageKind::Noncombat => DamageKindFilter::Noncombat,
    }
}

fn reject(detail: &str) -> Failure {
    step("prevent-damage", detail)
}

/// "Prevent all [combat] damage that would be dealt this turn [by matching
/// or announced sources]" and "prevent the next N damage that would be
/// dealt to [an announced recipient] this turn". Source choices, "next
/// time" shields, recipient lists, conditions and riders reject.
pub(super) fn lower(
    cx: &mut StepCx<'_>,
    prevention: &ast::DamagePrevention,
    duration: &SourceDuration,
) -> Result<Instruction, Failure> {
    uncounted(cx)?;
    match typed::<ast::Duration>(duration, "duration")? {
        ast::Duration::ThisTurn(_) | ast::Duration::EndOfTurn(_) => {}
        other => return Err(operand("duration", variant(&other))),
    }
    let p = prevention;
    if p.incoming_amount.is_some()
        || p.condition.is_some()
        || p.optional.is_some()
        || p.bindings.is_some()
        || p.after.is_some()
        || p.involving.is_some()
    {
        return Err(reject("qualifier"));
    }
    use ast::DamagePreventionEventScopeSources::Simultaneous;
    if p.event_scope.sources != Simultaneous || p.event_scope.recipients != Simultaneous {
        return Err(reject("event-scope"));
    }
    let shield = match &*p.shield {
        ast::DamagePreventionShield::Events(events)
            if events.occurrence == ast::EffectCreateDelayedTriggerOccurrence::Each
                && matches!(*events.prevent, ast::PreventionQuantity::All(_)) =>
        {
            PreventionShield::All
        }
        ast::DamagePreventionShield::DamageBudget(budget) if budget.allocation.is_none() => {
            PreventionShield::Next {
                amount: fixed_positive_amount(&budget.amount)?,
            }
        }
        _ => return Err(reject("shield")),
    };
    let to = match p.to.as_deref() {
        None => None,
        Some([ast::AmountCounterCountOn::ObjectRef(what)]) => {
            let slot = match &**what {
                ast::ObjectRef::AnyTarget(target)
                    if target.count.is_none()
                        && target.up_to.is_none()
                        && target.other.is_none()
                        && target.random.is_none()
                        && target.chosen_by.is_none()
                        && target.excluding.is_none() =>
                {
                    bind_target(
                        cx.program,
                        cx.slots,
                        target.selection_id.as_deref().unwrap_or_default(),
                        TargetDomain::AnyDamageTarget,
                    )?
                }
                ast::ObjectRef::Target(_) => bind_creature_target(cx.program, cx.slots, what)?,
                _ => return Err(reject("to")),
            };
            Some(kir::Ref::Target { slot })
        }
        Some(_) => return Err(reject("to")),
    };
    let by = match p.by.as_deref() {
        None => None,
        Some(ast::DamagePreventionBy::ObjectRef(what)) => Some(match &**what {
            ast::ObjectRef::Target(_) => PreventionSources::One {
                object: kir::Ref::Target {
                    slot: bind_creature_target(cx.program, cx.slots, what)?,
                },
            },
            ast::ObjectRef::Each(each) => PreventionSources::Matching {
                filter: lower_declaration_filter(&each.filter)?,
            },
            ast::ObjectRef::All(all) => PreventionSources::Matching {
                filter: lower_declaration_filter(&all.filter)?,
            },
            _ => return Err(reject("by")),
        }),
        Some(_) => return Err(reject("by:choice")),
    };
    Ok(cx.act(Verb::PreventDamage {
        prevention: ResolvedPrevention {
            kind: damage_kind(p.kind),
            shield,
            to,
            by,
        },
        until: kir::Until::EndOfTurn,
    }))
}
