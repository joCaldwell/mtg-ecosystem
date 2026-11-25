//! Event table: self events whose runtime condition does not depend on which
//! permanent type the source is. Every other event keeps the shared trigger
//! lowerer, so a row here never changes an already supported trigger.
//!
//! CR 603.6a: enters-the-battlefield abilities trigger when the permanent
//! enters; the condition names the object, not one of its card types.
//! CR 603.2: "a land you control enters" is read from the event's resulting
//! battlefield, including this source when it is the entering land.
//! CR 701.20a: "becomes tapped" is the source's own untapped-to-tapped change.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

pub(super) fn lower(card: &Card<'_>, trigger: &SourceTrigger) -> Option<EventPattern> {
    use kir::AttachmentKind::{Enchanted, Equipped};
    // CR 301.5, 303.4: the creature the source is attached to.
    enum Host {
        Attacks,
        Dies,
        CombatDamageToPlayer,
    }
    use Host::{Attacks, CombatDamageToPlayer, Dies};
    let host = match trigger {
        SourceTrigger::Attacks {
            what: SourceAttacker::Equipped { noun },
        } => Some((Equipped, Attacks, noun)),
        SourceTrigger::Attacks {
            what: SourceAttacker::Enchanted { noun },
        } => Some((Enchanted, Attacks, noun)),
        SourceTrigger::Dies {
            what: TriggerObject::Equipped { noun },
        } => Some((Equipped, Dies, noun)),
        SourceTrigger::Dies {
            what: TriggerObject::Enchanted { noun },
        } => Some((Enchanted, Dies, noun)),
        SourceTrigger::DealsDamage {
            what,
            combat: Some(true),
            to,
            grouping: None,
        } if to == "any" => match what {
            SourceDamageObject::Equipped { noun } => Some((Equipped, CombatDamageToPlayer, noun)),
            SourceDamageObject::Enchanted { noun } => Some((Enchanted, CombatDamageToPlayer, noun)),
            _ => None,
        },
        _ => None,
    };
    if let Some((attachment, event, noun)) = host {
        // The printed host noun must match what the source attaches as.
        if noun != "creature" || !card.attaches_as(attachment) {
            return None;
        }
        let object = Pred::attached_creature();
        return Some(match event {
            Attacks => EventPattern::Attacks { attacker: object },
            Dies => EventPattern::Dies { object },
            CombatDamageToPlayer => EventPattern::DealsDamage {
                source: object,
                combat: true,
                recipient: PlayerExpr::EachPlayer,
            },
        });
    }
    match trigger {
        SourceTrigger::Enters {
            what: TriggerObject::This { noun },
        } if card.is_self(noun)
            // The shared lowerer already owns these two spellings.
            && !matches!(noun.as_str(), "creature" | "enchantment") =>
        {
            Some(EventPattern::self_enters())
        }
        SourceTrigger::Enters {
            what: TriggerObject::Filter { filter, count },
        } if filter.types == ["land"]
            && filter.control.as_deref() == Some("you")
            && !filter.other
            && !filter.non_token
            && filter.non_subtypes.is_empty()
            && filter.properties.is_none()
            && fixed_one(count) =>
        {
            Some(EventPattern::controlled_land_enters())
        }
        SourceTrigger::BecomesTapped {
            what: TriggerObject::This { noun },
        } if card.is_self(noun) => Some(EventPattern::self_becomes_tapped()),
        _ => None,
    }
}

/// The general event table: an enters, dies, put-into-a-graveyard-from-the-
/// battlefield or attacks event whose object is this permanent, its named
/// self, or one filtered object, and the same-kind alternatives of such
/// events ("this creature or another Elf you control"). The object filter
/// lowers term by term to a [`Pred`]; a printed term with no atom rejects.
/// `None` leaves the trigger to the reviewed lowerers' own rejection.
///
/// CR 603.2, 603.6a: an entering object is matched on the resulting
/// battlefield. CR 700.4, 603.10a: "dies" and "is put into a graveyard from
/// the battlefield" are one event, matched on the prior battlefield.
/// CR 508.3a: an attacking creature is matched as it is declared.
/// CR 201.5: "this" and the card's name are the source; "other" excludes it.
/// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
pub(super) fn general(
    card: &Card<'_>,
    trigger: &serde_json::Value,
) -> Result<Option<EventPattern>, Failure> {
    #[derive(Clone, Copy, PartialEq)]
    enum Kind {
        Enters,
        Dies,
        Attacks,
    }
    fn event(card: &Card<'_>, raw: &serde_json::Value) -> Result<Option<(Kind, Pred)>, Failure> {
        let Some(object) = raw.as_object() else {
            return Ok(None);
        };
        let (kind, extra): (Kind, &[(&str, serde_json::Value)]) =
            match object.get("trigger").and_then(|t| t.as_str()) {
                Some("enters") => (Kind::Enters, &[]),
                Some("dies") => (Kind::Dies, &[]),
                Some("attacks") => (Kind::Attacks, &[]),
                Some("put-into-graveyard") => (
                    Kind::Dies,
                    &[
                        ("from", serde_json::Value::Null),
                        ("to", serde_json::Value::Null),
                    ],
                ),
                _ => return Ok(None),
            };
        // Every other printed field (a grouping, an origin, an occurrence
        // count, a turn window) is a qualifier this table does not lower.
        if object.keys().any(|k| {
            !matches!(k.as_str(), "trigger" | "what") && !extra.iter().any(|(f, _)| f == k)
        }) {
            return Ok(None);
        }
        if !extra.is_empty()
            && (object.get("from") != Some(&serde_json::json!("battlefield"))
                || object.get("to") != Some(&serde_json::json!({"zone":"graveyard","owner":"any"})))
        {
            return Ok(None);
        }
        let Some(what) = object.get("what") else {
            return Ok(None);
        };
        let what: ast::ObjectRef = typed(what, "trigger.what")?;
        Ok(Some((kind, subject(card, &what)?)))
    }
    let mut parts = match trigger.get("trigger").and_then(|t| t.as_str()) {
        Some("any-of") => {
            let Some(events) = trigger.get("events").and_then(|e| e.as_array()) else {
                return Ok(None);
            };
            if trigger.as_object().is_some_and(|o| o.len() != 2) || events.len() < 2 {
                return Ok(None);
            }
            let mut parts = vec![];
            for e in events {
                let Some(part) = event(card, e)? else {
                    return Ok(None);
                };
                parts.push(part);
            }
            parts
        }
        _ => match event(card, trigger)? {
            Some(part) => vec![part],
            None => return Ok(None),
        },
    };
    let kind = parts[0].0;
    if parts.iter().any(|(k, _)| *k != kind) {
        return Ok(None);
    }
    let object = if parts.len() == 1 {
        parts.remove(0).1
    } else {
        // CR 603.2: one occurrence triggers the ability once, whichever
        // alternative its object satisfies.
        Pred(vec![Atom::AnyOf {
            alternatives: parts.into_iter().map(|(_, p)| p).collect(),
        }])
    };
    Ok(Some(match kind {
        Kind::Enters => EventPattern::Enters { object },
        Kind::Dies => EventPattern::Dies { object },
        Kind::Attacks => EventPattern::Attacks { attacker: object },
    }))
}

/// The object an event names: the source (by "this" noun or by name) or one
/// filtered object.
fn subject(card: &Card<'_>, what: &ast::ObjectRef) -> Result<Pred, Failure> {
    match what {
        ast::ObjectRef::This(this)
            if card.is_self(&this.noun) || card.names_own_subtype(&this.noun) =>
        {
            Ok(Pred::source())
        }
        ast::ObjectRef::SelfObject(_) => Ok(Pred::source()),
        ast::ObjectRef::Filter(f)
            if f.up_to.is_none()
                && f.group_constraints.is_none()
                && f.random.is_none()
                && matches!(&*f.count, ast::Amount::Fixed(n) if n.value == 1) =>
        {
            lower_event_filter(&f.filter)
        }
        other => Err(unsupported(
            "trigger.what",
            format!("event object {}", variant(other)),
        )),
    }
}

/// CR 603.4, 109.5: an intervening "if you control a [filter]" or "if you
/// control N or more [filter]" clause, read when the event occurs and again
/// as the ability resolves. The filter may say "other" (CR 201.5).
pub(super) fn board_intervening(
    condition: &serde_json::Value,
) -> Result<kir::Intervening, Failure> {
    let reject = || {
        unsupported(
            "abilities.condition",
            "unsupported complete intervening-if condition",
        )
    };
    let fields = condition.as_object().map(|o| o.len()).unwrap_or(0);
    if condition.get("who") != Some(&serde_json::json!({"player": "you"})) {
        return Err(reject());
    }
    let (filter, cmp, value) = match condition.get("condition").and_then(|c| c.as_str()) {
        Some("control") if fields == 3 => {
            let what = &condition["what"];
            if what.as_object().map(|o| o.len()) != Some(3)
                || what["ref"] != "filter"
                || what["count"] != serde_json::json!({"amount": "fixed", "value": 1})
            {
                return Err(reject());
            }
            (&what["filter"], Cmp::AtLeast, Quantity::from(1))
        }
        Some("control-count") if fields == 4 => {
            let (cmp, value) = control::comparison(&condition["comparison"])?;
            (&condition["of"], cmp, value)
        }
        _ => return Err(reject()),
    };
    if filter.get("control").is_some() || filter.get("zone").is_some() {
        return Err(reject());
    }
    let filter: ast::ObjectFilter = typed(filter, "filter")?;
    let mut pred = lower_event_filter(&filter)?;
    pred.0.push(Atom::ControlledBy {
        player: PlayerExpr::you(Context::Triggered),
    });
    let intervening = kir::Intervening::BattlefieldCount {
        filter: pred,
        cmp,
        value,
    };
    // The kernel form must name exactly this clause.
    match kir::Intervening::of(&intervening.condition()) {
        Ok(found) if found == intervening => Ok(intervening),
        _ => Err(reject()),
    }
}
