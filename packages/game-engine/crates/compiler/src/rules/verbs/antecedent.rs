//! The antecedent of a singular object pronoun (CR 608.2c, 608.2h, 400.7).
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
/// step named, make it ambiguous. With neither, "it" in a trigger on the
/// source itself means that event object (its last known information,
/// CR 603.10a).
pub(super) fn resolve(cx: &StepCx<'_>, object: &ast::ObjectRef) -> Result<Ref, Failure> {
    let (resolved, word) = bind(cx, object)?;
    Ok(record(cx, word, resolved))
}

/// The permanent a pronoun names when a step acts on it. A card or spell
/// target has become a new object once it moved (CR 400.7), so acting on
/// it through the pronoun rejects.
pub(super) fn permanent(cx: &StepCx<'_>, object: &ast::ObjectRef) -> Result<Ref, Failure> {
    let (resolved, word) = bind(cx, object)?;
    if let Ref::Target { slot } = resolved {
        if !matches!(
            cx.program.targets.get(slot as usize).map(target_class),
            Some(TargetClass::Object(Kind::Permanent(_)))
        ) {
            return Err(operand("object", "pronoun:nonpermanent_target"));
        }
        if moved(cx, slot) {
            return Err(operand("object", "pronoun:moved_target"));
        }
    }
    Ok(record(cx, word, resolved))
}

/// CR 400.7: an earlier instruction moved or countered the target, so a
/// pronoun acting on "it" would mean the new object, which no slot names.
fn moved(cx: &StepCx<'_>, slot: u32) -> bool {
    fn names(value: &serde_json::Value, slot: u32) -> bool {
        match value {
            serde_json::Value::Object(object) => {
                (object.get("ref").and_then(|r| r.as_str()) == Some("target")
                    && object.get("slot").and_then(|s| s.as_u64()) == Some(u64::from(slot)))
                    || object.values().any(|v| names(v, slot))
            }
            serde_json::Value::Array(items) => items.iter().any(|v| names(v, slot)),
            _ => false,
        }
    }
    fn moves(value: &serde_json::Value, slot: u32) -> bool {
        match value {
            serde_json::Value::Object(object) => {
                (matches!(
                    object.get("verb").and_then(|v| v.as_str()),
                    Some("move" | "counter")
                ) && names(value, slot))
                    || object.values().any(|v| moves(v, slot))
            }
            serde_json::Value::Array(items) => items.iter().any(|v| moves(v, slot)),
            _ => false,
        }
    }
    cx.program
        .instructions
        .iter()
        .any(|instruction| serde_json::to_value(instruction).is_ok_and(|value| moves(&value, slot)))
}

fn bind(cx: &StepCx<'_>, object: &ast::ObjectRef) -> Result<(Ref, &'static str), Failure> {
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
    for target in &cx.program.targets {
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
    let role = cx.player_binding.event_object_role();
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
    // An announced target is printed after the trigger's event subject, so
    // the one admissible target is the nearer antecedent; two are ambiguous.
    let resolved = match (targets.as_slice(), event, competing) {
        (_, _, true) | ([_, _, ..], ..) => {
            return Err(operand("object", "ambiguous_antecedent"));
        }
        ([one], ..) => *one,
        ([], Some(role), _) => Ref::EventObject { role },
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
