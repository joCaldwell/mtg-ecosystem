//! CR 601.2a/601.2i/611.2f/702.85: separately collect static and resolved
//! cascade grants on successful casts. The spell is the rules source;
//! the grantor is historical provenance (113.7a/113.10).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use std::borrow::Cow;
use std::sync::LazyLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "origin", rename_all = "snake_case", deny_unknown_fields)]
pub enum EffectiveTriggerAbility {
    DelayedChapter { chapter: u32, issuer: StackId },
    Intrinsic { index: u32 },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CascadeGrantProvenance {
    pub source_facts: ObjectBefore,
    pub source_handles: Vec<publish::StoredObjectHandle>,
}
fn trigger_grant_charge(
    ability: EffectiveTriggerAbility,
    witness: &TriggerEventWitness,
    grant: Option<&CascadeGrantProvenance>,
) -> usize {
    let history = match witness {
        TriggerEventWitness::DelayedSpellCast { .. }
        | TriggerEventWitness::DelayedCreatureEntered { .. } => {
            stored_history::payload_charge(witness)
        }
        TriggerEventWitness::ManaTapped { .. } => stored_history::payload_charge(witness),
        TriggerEventWitness::SpellCast { cast } => stored_history::payload_charge(&cast.history),
        _ => 0,
    };
    let temporary_origin = { 0 };
    history
        .saturating_add(grant.map_or(0, stored_history::payload_charge))
        .saturating_add(temporary_origin)
}
pub fn entry_grant_charge(entry: &StackInstance) -> usize {
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
impl RulesTxn {
    pub(crate) fn grant_payload_charge(&self) -> usize {
        let mut charge = 0usize;
        for registration in &self.state.rules.delayed_chapters {
            charge = charge.saturating_add(stored_history::payload_charge(registration));
        }
        // Cascade/owned-child frames already charge their complete payload.
        for t in &self.state.rules.pending_triggers {
            charge = charge.saturating_add(trigger_grant_charge(
                t.ability,
                &t.witness,
                t.grant.as_ref(),
            ));
        }
        let rules = &self.state.rules;
        for s in rules.stack.iter().chain(rules.detached.values()) {
            charge = charge.saturating_add(entry_grant_charge(s));
        }
        for effect in &self.state.rules.effects {}
        charge
    }

    pub(super) fn effective_trigger(
        &self,
        source: ObjectId,
        ability: EffectiveTriggerAbility,
    ) -> Result<Cow<'_, mtg_contracts::TriggeredAbility>, String> {
        match ability {
            EffectiveTriggerAbility::DelayedChapter { chapter, .. } => {
                let (condition, program) = self.delayed_chapter(source, chapter)?;
                Ok(Cow::Owned(mtg_contracts::TriggeredAbility {
                    functioning_zone: FunctioningZone::Battlefield,
                    condition,
                    intervening_if: None,
                    death_filter: None,
                    program: program.clone(),
                }))
            }
            EffectiveTriggerAbility::Intrinsic { index } => self
                .copiable_definition(source)?
                .triggered
                .get(index as usize)
                .map(Cow::Borrowed)
                .ok_or_else(|| "missing intrinsic triggered definition".into()),
        }
    }

    pub(super) fn validate_cascade_grant(
        &self,
        ability: EffectiveTriggerAbility,
        controller: PlayerId,
        witness: &TriggerEventWitness,
        grant: Option<&CascadeGrantProvenance>,
    ) -> Result<(), String> {
        if grant.is_some() {
            return Err("intrinsic trigger has grant provenance".into());
        }
        Ok(())
    }
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
pub fn retained_effect_frame<'a>(frames: &'a [Frame], mut frame: &'a Frame) -> &'a Frame {
    loop {
        frame = match frame {
            Frame::ManaAbility { activation } => &frames[activation.resume.0],
            Frame::DamageEffects { transaction } => match &transaction.continuation {
                DamageContinuation::Mana { activation, .. } => &frames[activation.resume.0],
                _ => return frame,
            },
            Frame::ManaSourceChoice { resume, .. }
            | Frame::Stabilize { resume, .. }
            | Frame::LegendRule { resume, .. } => &frames[resume.0],
            Frame::CommanderReturn { continuation, .. } => match continuation {
                CommanderContinuation::StateBased { resume }
                | CommanderContinuation::StateBasedBatch { resume, .. } => &frames[resume.0],
                _ => return frame,
            },
            _ => return frame,
        };
    }
}
