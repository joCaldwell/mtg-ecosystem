//! CR 301.5, 603.3a, 603.10a. The Equipment observes the declared event.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

impl Game {
    pub(super) fn observe_equipped(
        &self,
        equipment: &Object,
        condition: &TriggerCondition,
        event: &TriggerEventWitness,
    ) -> Option<TriggerEventWitness> {
        let target = event.subject()?;
        let timestamp = equipment.attachment_timestamp?;
        if equipment.attached_to != Some(target)
            || !self.target_is_creature(target)
            || !event_matches(condition, event)
        {
            return None;
        }
        Some(TriggerEventWitness::Equipped {
            attachment: AttachmentBefore {
                object: equipment.id,
                target: Some(target),
                timestamp: Some(timestamp),
            },
            occurrence: Box::new(event.clone()),
        })
    }

    pub(super) fn valid_equipped_witness(
        &self,
        source: ObjectId,
        condition: &TriggerCondition,
        attachment: &AttachmentBefore,
        event: &TriggerEventWitness,
    ) -> bool {
        if !event_matches(condition, event)
            || attachment.object != source
            || !attachment
                .timestamp
                .is_some_and(|t| t > 0 && t < self.state.rules.next_attachment_timestamp)
        {
            return false;
        }
        let Some(target) = attachment.target else {
            return false;
        };
        if target == source
            || event.subject() != Some(target)
            || !self.state.rules.cards.contains_key(&target.card)
        {
            return false;
        }
        match event {
            TriggerEventWitness::DamageDealt {
                damage,
                transaction,
                match_source,
                ..
            } => {
                match_source.is_none()
                    && damage.source.zone == Zone::Battlefield
                    && damage.source.characteristics.has_type(CardKind::Creature)
                    && self
                        .validate_characteristic_identity(
                            target,
                            damage.source.zone,
                            &damage.source.characteristics,
                        )
                        .is_ok()
                    && damage.source.default_controller.0 < 4
                    && damage.controller == damage.source.controller
                    && damage.controller.0 < 4
                    && self.state.rules.cards[&target.card].owner == damage.source.owner
                    && (damage.id.0 as usize) < self.damage_limit()
                    && transaction
                        .is_none_or(|id| id.0 < self.state.rules.next_damage_transaction_id)
            }
            TriggerEventWitness::Moved { movement } => {
                matches!(movement.destination,Zone::Graveyard{owner} if owner==self.state.rules.cards[&target.card].owner)
                    && movement.successor.card == target.card
                    && target.incarnation.checked_add(1) == Some(movement.successor.incarnation)
            }
            _ => false,
        }
    }
}

fn event_matches(condition: &TriggerCondition, event: &TriggerEventWitness) -> bool {
    match (condition, event) {
        (
            TriggerCondition::EquippedCombatDamage,
            TriggerEventWitness::DamageDealt {
                kind: DamageKind::Combat,
                damage,
                ..
            },
        ) => {
            damage.amount > Quantity::zero()
                && matches!(damage.recipient, DamageRecipient::Player { player } if player.0 < 4)
        }
        (TriggerCondition::EquippedDies, TriggerEventWitness::Moved { movement }) => {
            movement.from == Zone::Battlefield
                && matches!(movement.destination, Zone::Graveyard { .. })
        }
        _ => false,
    }
}
