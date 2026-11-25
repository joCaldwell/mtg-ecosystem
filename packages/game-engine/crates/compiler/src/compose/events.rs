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
