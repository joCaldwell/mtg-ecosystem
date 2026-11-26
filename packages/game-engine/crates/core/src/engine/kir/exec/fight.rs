//! CR 701.14a–d: check both current creatures, then prepare one noncombat
//! damage group from their powers. A creature fighting itself deals one
//! occurrence for twice its power.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use crate::engine::*;
use mtg_contracts::kir::Ref;

fn strikes(
    fighters: &[ObjectBefore; 2],
) -> impl Iterator<Item = (&ObjectBefore, ObjectId, Quantity)> {
    let [first, second] = fighters;
    [(first, second.object), (second, first.object)]
        .into_iter()
        .take(if first.object == second.object { 1 } else { 2 })
        .filter_map(|(source, target)| {
            let mut amount = source.characteristics.power.as_ref()?.clone();
            if amount <= Quantity::zero() {
                return None;
            }
            if source.object == target {
                amount.add(&amount.clone());
            }
            Some((source, target, amount))
        })
}

impl RulesTxn {
    fn fight_participant(
        &self,
        frame: &Resolving,
        fighter: &Ref,
    ) -> Result<Option<ObjectId>, String> {
        let entry = self.entry(frame.stack_id)?;
        Ok(match fighter {
            Ref::Source => Some(entry.source),
            Ref::Target { slot } if frame.legal_targets.get(*slot as usize) == Some(&true) => {
                self.bound_object_target(entry, *slot as usize).copied()
            }
            Ref::Target { .. } => None,
            _ => return Err("unsupported fight participant".into()),
        })
    }

    pub(in crate::engine) fn execute_fight(
        &mut self,
        mut frame: Resolving,
        first: &Ref,
        second: &Ref,
        block: u32,
    ) -> Result<(), String> {
        let current_creature = |fighter| {
            Ok::<_, String>(
                self.fight_participant(&frame, fighter)?
                    .filter(|object| self.is_battlefield_creature(*object)),
            )
        };
        let (Some(first), Some(second)) = (current_creature(first)?, current_creature(second)?)
        else {
            return self.finish_instruction(frame, InstructionOutcome::Damage { actual: None });
        };
        let fighters = Box::new([self.object_before(first)?, self.object_before(second)?]);
        let mut entries = vec![];
        for (source, target, amount) in strikes(&fighters) {
            entries.push(effects::PreparedDamage {
                source_appearance: None,
                source_handles: self.capture_trigger_source_handles(source.object)?,
                id: DamageId(entries.len() as u32),
                source: source.clone(),
                controller: source.controller,
                recipient: DamageRecipient::Creature { object: target },
                original_amount: amount.clone(),
                amount,
                applied: vec![],
            });
        }
        if entries.is_empty() {
            return self.finish_instruction(frame, InstructionOutcome::Damage { actual: None });
        }
        let cause = self.instruction_cause(&frame, block)?;
        frame.fight = Some(fighters);
        self.begin_damage(
            DamageKind::Noncombat,
            entries,
            cause,
            DamageContinuation::Instruction {
                frame: Box::new(frame),
            },
        )
    }

    pub(in crate::engine) fn validate_fight_damage(
        &self,
        tx: &DamageTransaction,
        frame: &Resolving,
        first: &Ref,
        second: &Ref,
        block: u32,
    ) -> Result<(), String> {
        let fighters = frame
            .fight
            .as_ref()
            .ok_or("missing frozen fight participants")?;
        for (reference, frozen) in [first, second].into_iter().zip(fighters.iter()) {
            if self.fight_participant(frame, reference)? != Some(frozen.object)
                || frozen.zone != Zone::Battlefield
                || !frozen.characteristics.has_type(CardKind::Creature)
                || frozen.characteristics.power.is_none()
                || self
                    .state
                    .rules
                    .cards
                    .get(&frozen.object.card)
                    .is_none_or(|c| c.owner != frozen.owner)
            {
                return Err("invalid frozen fight participant".into());
            }
            self.validate_fact_identity(frozen)?;
            if let Ok(current) = self.object_before(frozen.object)
                && !self.frozen_damage_source_matches(frozen, current.clone())
                && !tx.entries.iter().any(|d| {
                    &d.source == frozen && self.prepared_damage_source_matches(d, current.clone())
                })
            {
                return Err("changed fight participant".into());
            }
        }
        if fighters[0].object == fighters[1].object && fighters[0] != fighters[1] {
            return Err("inconsistent self-fight participant".into());
        }
        let expected = strikes(fighters).collect::<Vec<_>>();
        if tx.cause != self.instruction_cause(frame, block)?
            || tx.entries.len() != expected.len()
            || tx
                .entries
                .iter()
                .zip(expected)
                .any(|(d, (source, target, amount))| {
                    &d.source != source
                        || d.recipient != (DamageRecipient::Creature { object: target })
                        || d.original_amount != amount
                        || d.controller != source.controller
                        || d.source_handles.iter().enumerate().any(|(viewer, handle)| {
                            self.object_handle_id(PlayerId(viewer as u8), source.object)
                                .ok()
                                != Some(handle)
                        })
                })
        {
            return Err("unbound fight damage transaction".into());
        }
        Ok(())
    }
}
