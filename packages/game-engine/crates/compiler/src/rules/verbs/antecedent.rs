//! The antecedent of a singular object pronoun (CR 608.2c, 608.2h, 400.7,
//! 201.5, 700.2).
//!
//! "It" and "that <noun>" name one object the block already introduced:
//! an announced target, the trigger's event object, or the object
//! performing the step. Rust decides which from the typed facts it has:
//! each target's domain and filter, the event role and whether the event
//! object is the source, and which earlier printed steps named another
//! object. Exactly one admissible candidate binds; none or several reject
//! with a located diagnostic, never a positional guess.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use crate::reference_trace::{self, Bound};

/// What a "that <noun>" can name.
#[derive(Debug, PartialEq, Eq)]
enum Noun {
    /// "that creature", "that artifact", "that permanent": a battlefield object.
    Permanent(Vec<CardKind>),
    /// "that card": a card outside the battlefield.
    Card,
    /// "that spell": a spell on the stack.
    Spell,
}

impl Noun {
    fn of(that: &ast::ObjectRefThat) -> Result<Self, Failure> {
        if that.plural.is_some() {
            return Err(operand("object", "that:plural"));
        }
        let filter = &that.filter;
        let types = || -> Result<Vec<CardKind>, Failure> {
            filter
                .types
                .iter()
                .flatten()
                .map(|kind| card_kind(kind, "object"))
                .collect()
        };
        Ok(match (present_fields(filter).as_slice(), filter.cls) {
            (["types"], None) => Self::Permanent(types()?),
            (["cls"], Some(ast::ObjectFilterCls::Permanent)) => Self::Permanent(vec![]),
            (["cls"] | ["types", "cls"], Some(ast::ObjectFilterCls::Card)) => Self::Card,
            (["cls"], Some(ast::ObjectFilterCls::Spell)) => Self::Spell,
            _ => return Err(operand("object", "that:qualified")),
        })
    }

    fn admits(&self, kind: &Kind) -> bool {
        match (self, kind) {
            (Self::Permanent(nouns), Kind::Permanent(kinds)) => {
                nouns.is_empty() || kinds.is_empty() || nouns.iter().any(|n| kinds.contains(n))
            }
            (Self::Card, Kind::Card) | (Self::Spell, Kind::Spell) => true,
            _ => false,
        }
    }
}

/// What one candidate object is.
enum Kind {
    Permanent(Vec<CardKind>),
    Card,
    Spell,
}

/// How an announced target can be referred back to.
enum TargetClass {
    Object(Kind),
    /// Players or objects ("any target"): a permanent noun or "it" may mean it.
    Mixed,
    /// A player: no object pronoun means it.
    Player,
    /// Several cards, or an ability: any pronoun may mean it, none binds it.
    Other,
}

fn target_class(target: &TargetSpec) -> TargetClass {
    let filtered = |default: Vec<CardKind>| {
        Kind::Permanent(
            target
                .filter
                .as_ref()
                .filter(|filter| !filter.types.is_empty())
                .map_or(default, |filter| filter.types.clone()),
        )
    };
    match target.domain {
        TargetDomain::BattlefieldPermanent | TargetDomain::ControlledBattlefieldPermanent => {
            TargetClass::Object(filtered(vec![]))
        }
        TargetDomain::BattlefieldCreature
        | TargetDomain::ControlledBattlefieldCreature
        | TargetDomain::BlockingBattlefieldCreature => {
            TargetClass::Object(filtered(vec![CardKind::Creature]))
        }
        TargetDomain::CreatureOrPlaneswalker => {
            TargetClass::Object(filtered(vec![CardKind::Creature, CardKind::Planeswalker]))
        }
        TargetDomain::OwnGraveyardCreatureCard
        | TargetDomain::OwnGraveyardCard
        | TargetDomain::GraveyardCard { .. } => TargetClass::Object(Kind::Card),
        TargetDomain::StackSpell | TargetDomain::ControlledCreatureSpell => {
            TargetClass::Object(Kind::Spell)
        }
        TargetDomain::OpponentPlayer | TargetDomain::AnyPlayer => TargetClass::Player,
        TargetDomain::AnyDamageTarget | TargetDomain::PlayerOrPlaneswalker => TargetClass::Mixed,
        TargetDomain::PlayerGraveyardCards { .. }
        | TargetDomain::StackActivatedOrTriggeredAbility => TargetClass::Other,
    }
}

/// CR 608.2c: the one object a singular "it" or "that <noun>" names, or a
/// located rejection. "Its" in a step performed by an object means that
/// object. Otherwise the antecedent is the one announced target the noun
/// admits, else the trigger's event object of an admitted type other than
/// the source. Two admissible targets, or any untargeted object an earlier
/// step named, make it ambiguous. A bare "it" may also mean a source the
/// block names apart from its event object (CR 201.5): an earlier step's
/// "this creature" is nearer than the event object but competes with a
/// target, and the trigger condition's ("a spell that targets this
/// creature") competes with the event object. With neither, "it" in a
/// trigger on the source itself means that event object (its last known
/// information, CR 603.10a).
pub(super) fn resolve(cx: &StepCx<'_>, object: &ast::ObjectRef) -> Result<Ref, Failure> {
    let (resolved, word) = bind(cx, object, false)?;
    Ok(record(cx, word, resolved))
}

/// The permanent a pronoun names when a step acts on it. A cast spell is
/// not a permanent, so it is never a candidate (CR 110.1). A card or spell
/// target has become a new object once it moved, and so has any object an
/// earlier instruction moved, so acting on it through the pronoun rejects
/// (CR 400.7).
pub(super) fn permanent(cx: &StepCx<'_>, object: &ast::ObjectRef) -> Result<Ref, Failure> {
    let (resolved, word) = bind(cx, object, true)?;
    if let Ref::Target { slot } = resolved
        && !matches!(
            cx.program.targets.get(slot as usize).map(target_class),
            Some(TargetClass::Object(Kind::Permanent(_)))
        )
    {
        return Err(operand("object", "pronoun:nonpermanent_target"));
    }
    if moved(cx, &resolved) {
        return Err(operand(
            "object",
            match resolved {
                Ref::Target { .. } => "pronoun:moved_target",
                _ => "pronoun:moved_object",
            },
        ));
    }
    Ok(record(cx, word, resolved))
}

/// CR 400.7: an earlier instruction of the current mode moved, destroyed,
/// sacrificed or countered the object `named` (CR 701.8a, 701.21a, 701.6a
/// each move it), so a pronoun acting on "it" would mean the new object,
/// which no reference names.
fn moved(cx: &StepCx<'_>, named: &Ref) -> bool {
    let Ok(named) = serde_json::to_value(named) else {
        return true;
    };
    fn names(value: &serde_json::Value, named: &serde_json::Value) -> bool {
        match value {
            serde_json::Value::Object(object) => {
                value == named || object.values().any(|v| names(v, named))
            }
            serde_json::Value::Array(items) => items.iter().any(|v| names(v, named)),
            _ => false,
        }
    }
    fn moves(value: &serde_json::Value, named: &serde_json::Value) -> bool {
        match value {
            serde_json::Value::Object(object) => {
                (matches!(
                    object.get("verb").and_then(|v| v.as_str()),
                    Some("move" | "counter" | "destroy" | "sacrifice")
                ) && names(value, named))
                    || object.values().any(|v| moves(v, named))
            }
            serde_json::Value::Array(items) => items.iter().any(|v| moves(v, named)),
            _ => false,
        }
    }
    cx.program
        .instructions
        .get(cx.floor.instructions..)
        .unwrap_or_default()
        .iter()
        .any(|instruction| {
            serde_json::to_value(instruction).is_ok_and(|value| moves(&value, &named))
        })
}

/// `acting`: the step acts on the pronoun's object as a permanent.
fn bind(
    cx: &StepCx<'_>,
    object: &ast::ObjectRef,
    acting: bool,
) -> Result<(Ref, &'static str), Failure> {
    let (noun, word) = match object {
        ast::ObjectRef::It(_) => (None, "it"),
        ast::ObjectRef::That(that) => (Some(Noun::of(that)?), "that"),
        other => return Err(operand("object", variant(other))),
    };
    let admits = |kind: &Kind| noun.as_ref().is_none_or(|noun| noun.admits(kind));
    if noun.is_none()
        && let Some(subject) = cx.subject
    {
        return Ok((subject, word));
    }
    let mut targets = Vec::new();
    let mut competing = cx.prior.other || cx.pending.other || cx.pending.object_target;
    for target in cx
        .program
        .targets
        .get(cx.floor.targets..)
        .unwrap_or_default()
    {
        match target_class(target) {
            TargetClass::Object(kind) if admits(&kind) => {
                targets.push(Ref::Target { slot: target.slot })
            }
            TargetClass::Object(_) | TargetClass::Player => {}
            TargetClass::Mixed => {
                competing |= noun
                    .as_ref()
                    .is_none_or(|noun| matches!(noun, Noun::Permanent(_)));
            }
            TargetClass::Other => competing = true,
        }
    }
    // CR 201.5: the source is "this <type>" or its name in Oracle text, so
    // only a bare "it" refers back to it; "that <noun>" means another
    // object. When the event object is the source, it is that object.
    let bare = noun.is_none() && !cx.event_is_source;
    let step_source = bare && (cx.prior.source || cx.pending.source);
    let source = step_source || bare && cx.source_named;
    // A permanent action never acts on a spell on the stack (CR 110.1).
    let role = cx
        .player_binding
        .event_object_role()
        .filter(|role| !(acting && *role == kir::EventObjectRole::CastSpell));
    let event = role.filter(|_| !cx.event_is_source).filter(|role| {
        admits(&if *role == kir::EventObjectRole::CastSpell {
            Kind::Spell
        } else {
            Kind::Permanent(
                CardKind::CARD_TYPES
                    .into_iter()
                    .filter(|kind| cx.event_types & (1 << kind.card_type_index()) != 0)
                    .collect(),
            )
        })
    });
    // An announced target, or an earlier step's "this creature", is printed
    // after the trigger's event subject, so it is the nearer antecedent. Two
    // targets, or a target beside such a step, are ambiguous; so is the
    // event object beside a source its own condition names.
    let resolved = match (targets.as_slice(), event, competing) {
        (_, _, true) | ([_, _, ..], ..) => {
            return Err(operand("object", "ambiguous_antecedent"));
        }
        ([_], ..) if step_source => return Err(operand("object", "ambiguous_antecedent")),
        ([one], ..) => *one,
        ([], ..) if step_source => Ref::Source,
        ([], Some(_), _) if source => return Err(operand("object", "ambiguous_antecedent")),
        ([], Some(role), _) => Ref::EventObject { role },
        ([], None, _) if source => Ref::Source,
        ([], None, _) => match role {
            Some(role) if noun.is_none() && cx.event_is_source => Ref::EventObject { role },
            _ => return Err(operand("object", "unbound_antecedent")),
        },
    };
    Ok((resolved, word))
}

/// The announced target a pronoun names, for an operand that reads only
/// targets ("its controller" after an announced removal).
pub(super) fn target(cx: &StepCx<'_>, object: &ast::ObjectRef) -> Result<Ref, Failure> {
    match resolve(cx, object)? {
        target @ Ref::Target { .. } => Ok(target),
        _ => Err(operand("player", "controller:unbound_target")),
    }
}

/// Report a bound pronoun to the sweep-time differential, when enabled.
pub(super) fn record(cx: &StepCx<'_>, word: &'static str, resolved: Ref) -> Ref {
    reference_trace::record(|| Bound {
        block: cx.source_block,
        word,
        resolved: match resolved {
            Ref::Target { slot } => reference_trace::Resolved::Target {
                selection: cx
                    .program
                    .targets
                    .get(slot as usize)
                    .map(|target| target.selection_id.clone())
                    .unwrap_or_default(),
            },
            Ref::EventObject { role } => reference_trace::Resolved::EventObject {
                role: format!("{role:?}"),
                is_source: cx.event_is_source,
            },
            Ref::Source => reference_trace::Resolved::Source,
            other => reference_trace::Resolved::Other {
                reference: format!("{other:?}"),
            },
        },
    });
    resolved
}
