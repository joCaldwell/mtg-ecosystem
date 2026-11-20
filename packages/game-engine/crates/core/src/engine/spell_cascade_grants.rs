//! CR 601.2a/601.2i/611.2f/702.85: separately collect static and resolved
//! cascade grants on successful casts. The spell is the rules source;
//! the grantor is historical provenance (113.7a/113.10).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use std::borrow::Cow;
use std::sync::LazyLock;

pub(super) const HAND_CAST_CASCADE_BLOCK: u32 = 1;
static HAND_CAST_CASCADE_PROGRAM: LazyLock<SpellProgram> =
    LazyLock::new(|| mtg_contracts::canonical_cascade_program(HAND_CAST_CASCADE_BLOCK));

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
    TemporaryGrant {
        effect: EffectId,
        grant_source: ObjectId,
        triggered_ability_index: u32,
        grant_stack_id: StackId,
        turn: u64,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CascadeGrantProvenance {
    pub source_facts: ObjectBefore,
    pub source_handles: Vec<ObjectHandle>,
}
fn trigger_grant_charge(
    ability: EffectiveTriggerAbility,
    witness: &TriggerEventWitness,
    grant: Option<&CascadeGrantProvenance>,
) -> usize {
    let history = match witness {
        TriggerEventWitness::SpellCast { cast } => stored_history::payload_charge(&cast.history),
        _ => 0,
    };
    let temporary_origin = if matches!(ability, EffectiveTriggerAbility::TemporaryGrant { .. }) {
        stored_history::payload_charge(&ability)
    } else {
        0
    };
    history
        .saturating_add(grant.map_or(0, stored_history::payload_charge))
        .saturating_add(temporary_origin)
}
pub(super) fn entry_grant_charge(entry: &StackInstance) -> usize {
    match &entry.kind {
        StackKind::Triggered {
            ability,
            witness,
            grant,
            ..
        } => trigger_grant_charge(*ability, witness, grant.as_ref()),
        _ => entry
            .hand_cast_permission
            .as_deref()
            .map_or(0, stored_history::payload_charge),
    }
}
impl Game {
    pub(super) fn grant_payload_charge(&self) -> usize {
        let mut charge = 0usize;
        // Cascade/owned-child frames already charge their complete payload.
        for t in &self.state.rules.pending_triggers {
            charge = charge.saturating_add(trigger_grant_charge(
                t.ability,
                &t.witness,
                t.grant.as_ref(),
            ));
        }
        for s in &self.state.rules.stack {
            charge = charge.saturating_add(entry_grant_charge(s));
        }
        if let Some(entry) = retained_resolving_entry(&self.state.rules.frame) {
            charge = charge.saturating_add(entry_grant_charge(entry));
        }
        for effect in &self.state.rules.effects {
            if matches!(effect.kind, ResolvedEffectKind::GrantHandCastCascade { .. }) {
                charge = charge.saturating_add(stored_history::payload_charge(effect));
            }
        }
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
    pub(super) fn temporary_grant_program(
        &self,
        grant_source: ObjectId,
        index: u32,
    ) -> Result<&SpellProgram, String> {
        let trigger = self
            .definition(grant_source.card)?
            .triggered
            .get(index as usize)
            .ok_or("missing hand cast grant definition")?;
        if trigger.functioning_zone != FunctioningZone::Battlefield
            || !matches!(
                trigger.condition,
                TriggerCondition::DamageDealt {
                    source: DamageTriggerSource::Source {},
                    kind: mtg_contracts::TriggerDamageKind::Combat,
                    recipient: mtg_contracts::DamageTriggerRecipient::Player,
                }
            )
            || !matches!(
                trigger.program.instructions.as_slice(),
                [Instruction::GrantHandCastCascade {
                    source_block: HAND_CAST_CASCADE_BLOCK,
                }]
            )
        {
            return Err("invalid hand cast grant definition".into());
        }
        Ok(&HAND_CAST_CASCADE_PROGRAM)
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
            EffectiveTriggerAbility::Granted { .. }
            | EffectiveTriggerAbility::TemporaryGrant { .. } => {
                Ok(Cow::Owned(mtg_contracts::TriggeredAbility {
                    functioning_zone: FunctioningZone::Stack,
                    condition: TriggerCondition::SelfSpellCast,
                    intervening_if: None,
                    death_filter: None,
                    program: self.effective_cascade_program(ability)?.clone(),
                }))
            }
        }
    }
    pub(super) fn effective_cascade_program(
        &self,
        ability: EffectiveTriggerAbility,
    ) -> Result<&SpellProgram, String> {
        match ability {
            EffectiveTriggerAbility::Granted {
                grant_source,
                static_ability_index,
            } => self.grant_program(grant_source, static_ability_index),
            EffectiveTriggerAbility::TemporaryGrant {
                grant_source,
                triggered_ability_index,
                ..
            } => self.temporary_grant_program(grant_source, triggered_ability_index),
            EffectiveTriggerAbility::Intrinsic { .. } => Err("expected cascade grant".into()),
        }
    }
    pub(super) fn validate_cascade_grant(
        &self,
        ability: EffectiveTriggerAbility,
        controller: PlayerId,
        witness: &TriggerEventWitness,
        grant: Option<&CascadeGrantProvenance>,
    ) -> Result<(), String> {
        if matches!(ability, EffectiveTriggerAbility::Intrinsic { .. }) {
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
        let (grant_source, static_ability_index) = match ability {
            EffectiveTriggerAbility::Granted {
                grant_source,
                static_ability_index,
            } => (grant_source, static_ability_index),
            EffectiveTriggerAbility::TemporaryGrant {
                effect,
                grant_source,
                triggered_ability_index,
                grant_stack_id,
                turn,
            } => {
                self.temporary_grant_program(grant_source, triggered_ability_index)?;
                self.validate_grant_source_facts(grant_source, fact, &grant.source_handles)?;
                if grant_stack_id.0 >= cast.stack_id.0
                    || effect.0 >= self.state.rules.next_effect_id
                    || !hand_cast_eligible(controller, turn, cast)
                {
                    return Err("invalid hand cast cascade provenance".into());
                }
                if let Some(active) = self.state.rules.effects.iter().find(|row| row.id == effect)
                    && (active.origin.stack_id != grant_stack_id
                        || active.origin.source != grant_source
                        || active.origin.controller != controller
                        || active.origin.source_handles != grant.source_handles
                        || active.expires_in_cleanup_of_turn != turn
                        || !matches!(&active.kind, ResolvedEffectKind::GrantHandCastCascade {
                            triggered_ability_index: index, source_facts,
                        } if *index == triggered_ability_index && source_facts.as_ref() == fact))
                {
                    return Err("hand cast cascade provenance disagrees with its effect".into());
                }
                // Accepted-input replay proves that this exact effect existed
                // at cast time, independently of the grantor's current zone.
                return Ok(());
            }
            EffectiveTriggerAbility::Intrinsic { .. } => unreachable!(),
        };
        let descriptor = self
            .definition(grant_source.card)?
            .static_abilities
            .get(static_ability_index as usize)
            .ok_or("missing cascade grant descriptor")?;
        let StaticEffect::GrantSpellCascade { eligibility, .. } = &descriptor.effect else {
            return Err("wrong cascade grant descriptor".into());
        };
        self.grant_program(grant_source, static_ability_index)?;
        self.validate_grant_source_facts(grant_source, fact, &grant.source_handles)?;
        let cost = &cast.spell.characteristics.mana_cost;
        let mut mana_value = cost.generic.clone();
        mana_value.add(&Quantity::from(cost.colored.len() as i64));
        if self.definition(cast.spell.object.card)?.x_mana_cost {
            mana_value.add(cast.x.as_ref().ok_or("missing granted spell X")?);
        }
        if fact.controller != controller
            || controller != cast.caster
            || !eligible(eligibility, &mana_value, cast)
        {
            return Err("invalid cascade grant provenance".into());
        }
        // Deliberately no current grantor existence/controller test. Accepted
        // input replay proves the complete event-time grant set and its facts.
        Ok(())
    }
    pub(super) fn validate_grant_source_facts(
        &self,
        source: ObjectId,
        fact: &ObjectBefore,
        handles: &[ObjectHandle],
    ) -> Result<(), String> {
        if fact.object != source
            || fact.zone != Zone::Battlefield
            || fact.controller.0 >= 4
            || fact.default_controller.0 >= 4
            || fact.owner.0 >= 4
            || handles.len() != 4
            || self
                .state
                .rules
                .cards
                .get(&source.card)
                .is_none_or(|c| c.owner != fact.owner)
        {
            return Err("invalid cascade grant source".into());
        }
        self.validate_characteristic_identity(source, fact.zone, &fact.characteristics)
    }
}

pub(super) fn hand_cast_eligible(
    controller: PlayerId,
    turn: u64,
    cast: &triggers::CastOccurrence,
) -> bool {
    cast.caster == controller
        && cast.cast_from == (Zone::Hand { owner: controller })
        && cast.history.turn_number == turn
}

/// Shared live/historical predicate; historical validation never consults today's ledger.
pub(super) fn eligible(
    eligibility: &mtg_contracts::SpellCascadeEligibility,
    mana_value: &Quantity,
    cast: &triggers::CastOccurrence,
) -> bool {
    match eligibility {
        mtg_contracts::SpellCascadeEligibility::ManaValueAtLeast { minimum } => {
            mana_value >= minimum
        }
        mtg_contracts::SpellCascadeEligibility::FirstCastEachTurn {} => cast.history.first_overall,
        mtg_contracts::SpellCascadeEligibility::FirstCastFromExileEachTurn {} => {
            cast.history.first_from_exile
        }
    }
}

/// Unwrap owning continuations without copying or mutating them.
pub(super) fn retained_effect_frame(mut frame: &Frame) -> &Frame {
    loop {
        frame = match frame {
            Frame::ManaAbility { activation } => &activation.resume,
            Frame::DamageEffects { transaction } | Frame::DamageAllocation { transaction, .. } => {
                match transaction.continuation.as_ref() {
                    DamageContinuation::Mana { activation, .. } => &activation.resume,
                    _ => return frame,
                }
            }
            Frame::ManaSourceChoice { resume, .. }
            | Frame::Stabilize { resume, .. }
            | Frame::LegendRule { resume, .. } => resume,
            Frame::CommanderReturn { continuation, .. } => match continuation.as_ref() {
                CommanderContinuation::StateBased { resume }
                | CommanderContinuation::StateBasedBatch { resume, .. } => resume,
                _ => return frame,
            },
            _ => return frame,
        };
    }
}
pub(super) fn retained_resolving_entry(frame: &Frame) -> Option<&StackInstance> {
    match retained_effect_frame(frame) {
        Frame::LandEntry {
            origin: EntryOrigin::Search { frame },
            ..
        }
        | Frame::Resolve { frame }
        | Frame::ResolutionChoice { frame, .. }
        | Frame::ForcedSacrifice { frame, .. }
        | Frame::OptionalDamage { frame } => Some(&frame.spell),
        Frame::CounterPlacement { transaction } => Some(&transaction.frame.spell),
        Frame::DamageEffects { transaction } | Frame::DamageAllocation { transaction, .. } => {
            match transaction.continuation.as_ref() {
                DamageContinuation::Instruction { frame } => Some(&frame.spell),
                DamageContinuation::Combat | DamageContinuation::Mana { .. } => None,
            }
        }
        Frame::CommanderReturn { continuation, .. } => match continuation.as_ref() {
            CommanderContinuation::GraveyardShuffle { frame, .. }
            | CommanderContinuation::HandReturn { frame, .. }
            | CommanderContinuation::LibrarySelect { frame, .. }
            | CommanderContinuation::BoardReturn { frame, .. }
            | CommanderContinuation::SpellMove { frame, .. }
            | CommanderContinuation::ResolutionDraw { frame, .. } => Some(&frame.spell),
            // CascadeBottom is charged in full by cascade_charge.
            _ => None,
        },
        // Cascade and owned child announcements are charged in full, once.
        _ => None,
    }
}
