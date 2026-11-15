//! Live CR 601.2i static grants. Rules source and controller belong to the cast
//! spell; the battlefield grantor is historical provenance only (113.7a/113.10).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use std::borrow::Cow;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "origin", rename_all = "snake_case", deny_unknown_fields)]
pub enum EffectiveTriggerAbility {
    Intrinsic {
        index: u32,
    },
    Granted {
        grant_source: ObjectId,
        static_ability_index: u32,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CascadeGrantProvenance {
    pub source_facts: ObjectBefore,
    pub source_handles: Vec<ObjectHandle>,
}
impl Game {
    pub(super) fn grant_payload_charge(&self) -> usize {
        let pending = self
            .state
            .rules
            .pending_triggers
            .iter()
            .filter_map(|t| t.grant.as_ref());
        let placed = self.state.rules.stack.iter().filter_map(|s| match &s.kind {
            StackKind::Triggered { grant, .. } => grant.as_ref(),
            _ => None,
        });
        let mut charge = pending.chain(placed).fold(0usize, |n, g| {
            n.saturating_add(stored_history::payload_charge(g))
        });
        if let Frame::Resolve { frame } = self.underlying_mana_frame(&self.state.rules.frame)
            && let StackKind::Triggered {
                grant: Some(grant), ..
            } = &frame.spell.kind
        {
            charge = charge.saturating_add(stored_history::payload_charge(grant));
        }
        // Cascade/owned-child frames already charge their complete payload.
        charge
    }

    pub(super) fn grant_program(
        &self,
        grant_source: ObjectId,
        index: u32,
    ) -> Result<&SpellProgram, String> {
        let ability = self
            .definition(grant_source.card)?
            .static_abilities
            .get(index as usize)
            .ok_or("missing static grant definition")?;
        match &ability.effect {
            StaticEffect::GrantSpellCascade { program, .. }
                if ability.functioning_zone == FunctioningZone::Battlefield
                    && mtg_contracts::is_canonical_cascade_program(program) =>
            {
                Ok(program)
            }
            _ => Err("invalid static grant program".into()),
        }
    }
    pub(super) fn effective_trigger(
        &self,
        source: ObjectId,
        ability: EffectiveTriggerAbility,
    ) -> Result<Cow<'_, mtg_contracts::TriggeredAbility>, String> {
        match ability {
            EffectiveTriggerAbility::Intrinsic { index } => self
                .definition(source.card)?
                .triggered
                .get(index as usize)
                .map(Cow::Borrowed)
                .ok_or_else(|| "missing intrinsic triggered definition".into()),
            EffectiveTriggerAbility::Granted {
                grant_source,
                static_ability_index,
            } => Ok(Cow::Owned(mtg_contracts::TriggeredAbility {
                functioning_zone: FunctioningZone::Stack,
                condition: TriggerCondition::SelfSpellCast,
                intervening_if: None,
                death_filter: None,
                program: self
                    .grant_program(grant_source, static_ability_index)?
                    .clone(),
            })),
        }
    }
    pub(super) fn validate_cascade_grant(
        &self,
        ability: EffectiveTriggerAbility,
        controller: PlayerId,
        witness: &TriggerEventWitness,
        grant: Option<&CascadeGrantProvenance>,
    ) -> Result<(), String> {
        let EffectiveTriggerAbility::Granted {
            grant_source,
            static_ability_index,
        } = ability
        else {
            return if grant.is_none() {
                Ok(())
            } else {
                Err("intrinsic trigger has grant provenance".into())
            };
        };
        let grant = grant.ok_or("missing cascade grant provenance")?;
        let TriggerEventWitness::SpellCast { cast } = witness else {
            return Err("grant without cast occurrence".into());
        };
        let fact = &grant.source_facts;
        let descriptor = self
            .definition(grant_source.card)?
            .static_abilities
            .get(static_ability_index as usize)
            .ok_or("missing cascade grant descriptor")?;
        let StaticEffect::GrantSpellCascade {
            mana_value_at_least,
            ..
        } = &descriptor.effect
        else {
            return Err("wrong cascade grant descriptor".into());
        };
        self.grant_program(grant_source, static_ability_index)?;
        self.validate_characteristic_identity(grant_source, &fact.characteristics)?;
        let cost = &cast.spell.characteristics.mana_cost;
        let mut mana_value = cost.generic.clone();
        mana_value.add(&Quantity::from(cost.colored.len() as i64));
        if self.definition(cast.spell.object.card)?.x_mana_cost {
            mana_value.add(cast.x.as_ref().ok_or("missing granted spell X")?);
        }
        if fact.object != grant_source
            || fact.zone != Zone::Battlefield
            || fact.controller != controller
            || controller != cast.caster
            || fact.default_controller.0 >= 4
            || fact.owner.0 >= 4
            || grant.source_handles.len() != 4
            || mana_value < *mana_value_at_least
            || self
                .state
                .rules
                .cards
                .get(&grant_source.card)
                .is_none_or(|c| c.owner != fact.owner)
        {
            return Err("invalid cascade grant provenance".into());
        }
        // Deliberately no current grantor existence/controller test. Accepted
        // input replay proves the complete event-time grant set and its facts.
        Ok(())
    }
}
