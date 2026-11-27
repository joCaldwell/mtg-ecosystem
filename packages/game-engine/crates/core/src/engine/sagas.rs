//! Saga rules. CR 505.4, 704.5s, 714.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use mtg_contracts::kir::{CounterKind, EventPattern};

impl RulesTxn {
    /// CR 714.2d: only the chapter abilities the Saga currently has count.
    fn final_chapter(&self, object: &Object, sources: Option<&[CardId]>) -> Result<u32, String> {
        if !object.present_on_battlefield() || object.face_down {
            return Ok(0);
        }
        let definition = self.copiable_definition(object.id)?;
        if !definition.characteristics.has_subtype("Saga")
            || !self
                .structural_characteristics(object.id)?
                .has_subtype("Saga")
            || self.printed_abilities_lost_with_sources(object.id, sources)?
        {
            return Ok(0);
        }
        Ok(definition
            .triggered
            .iter()
            .filter_map(|a| match a.condition.pattern() {
                EventPattern::SagaChapter { number } => Some(*number),
                _ => None,
            })
            .max()
            .unwrap_or(0))
    }

    /// CR 505.4, 714.3c: one simultaneous placement before priority.
    pub(super) fn main_phase_lore_group(&self) -> Result<Option<EventGroup>, String> {
        let Some(player) = self.state.rules.active else {
            return Ok(None);
        };
        if self.state.rules.turn.step != TurnStep::PrecombatMain
            || !matches!(self.state.rules.frame, Frame::EnterStep)
        {
            return Err("lore turn action outside precombat main entry".into());
        }
        let sources = self.static_source_index();
        let mut occurrences = vec![];
        for object in self
            .state
            .rules
            .objects
            .values()
            .filter(|o| o.controller == player)
        {
            if self.final_chapter(object, Some(&sources))? != 0 {
                occurrences.push(ProposedEvent::PutCounters {
                    object: object.id,
                    counters: vec![CounterAmount {
                        counter: CounterKind::Lore,
                        amount: Quantity::from(1),
                    }],
                });
            }
        }
        Ok((!occurrences.is_empty()).then_some(EventGroup::Simultaneous { occurrences }))
    }

    /// CR 704.5s, 714.4: any chapter still waiting for placement or on the
    /// stack protects its source incarnation, even when its abilities change.
    pub(super) fn saga_sacrifice(
        &self,
        object: &Object,
        sources: Option<&[CardId]>,
    ) -> Result<Option<ProposedEvent>, String> {
        let lore = object.counter_count(&CounterKind::Lore);
        if lore == Quantity::zero() {
            return Ok(None);
        }
        let final_chapter = self.final_chapter(object, sources)?;
        let chapter_waiting = || {
            self.state.rules.pending_triggers.iter().any(|t| {
                t.source == object.id && matches!(t.witness, TriggerEventWitness::LorePlaced { .. })
            }) || self.state.rules.stack.iter().any(|entry| {
                entry.source == object.id
                    && matches!(
                        entry.kind,
                        StackKind::Triggered {
                            witness: TriggerEventWitness::LorePlaced { .. },
                            ..
                        }
                    )
            })
        };
        Ok((final_chapter != 0
            && lore >= Quantity::from(i64::from(final_chapter))
            && !chapter_waiting())
        .then_some(ProposedEvent::Sacrifice {
            object: object.id,
            player: object.controller,
        }))
    }
}

/// CR 714.2b: a chapter's intervening threshold describes the occurrence.
/// It is not rechecked when the stack invocation resolves.
pub(super) fn crosses_chapter(previous: &Quantity, current: &Quantity, number: u32) -> bool {
    let chapter = Quantity::from(i64::from(number));
    !previous.is_negative() && *previous < chapter && chapter <= *current
}

impl RulesTxn {
    fn lore_removal_recipient(
        &self,
        frame: &Resolving,
        slot: u32,
    ) -> Result<Option<ObjectId>, String> {
        let entry = self.entry(frame.stack_id)?;
        Ok(self
            .bound_object_target(entry, slot as usize)
            .copied()
            .filter(|object| {
                frame.legal_targets.get(slot as usize) == Some(&true)
                    && self.current(*object).is_ok_and(|o| {
                        o.present_on_battlefield()
                            && o.counter_count(&CounterKind::Lore) > Quantity::zero()
                    })
            }))
    }

    pub(super) fn authorize_lore_removal(
        &self,
        cause: &BatchCause,
        group: &EventGroup,
    ) -> Result<(), String> {
        let Frame::Resolve { frame } = &self.state.rules.frame else {
            return Err("lore removal requires a resolving instruction".into());
        };
        self.validate_resolving(frame)?;
        let entry = self.entry(frame.stack_id)?;
        let Some(
            kernel!(RemoveLoreCounter { from: mtg_contracts::kir::Ref::Target { slot } } @ block),
        ) = self
            .program_for(entry)?
            .instructions
            .get(frame.cursor as usize)
        else {
            return Err("missing lore removal instruction".into());
        };
        let EventGroup::Occurrence {
            proposed: ProposedEvent::RemoveLoreCounter { object },
        } = group
        else {
            return Err("lore removal requires a single recipient".into());
        };
        if *cause != self.instruction_cause(frame, *block)?
            || self.lore_removal_recipient(frame, *slot)? != Some(*object)
        {
            return Err("lore removal must use its legal target and instruction".into());
        }
        Ok(())
    }

    pub(super) fn remove_lore_counter(
        &mut self,
        frame: Resolving,
        slot: u32,
        source_block: u32,
    ) -> Result<(), String> {
        let actual = if let Some(object) = self.lore_removal_recipient(&frame, slot)? {
            Some(self.commit(
                ProposedEvent::RemoveLoreCounter { object },
                self.instruction_cause(&frame, source_block)?,
            )?)
        } else {
            None
        };
        self.finish_instruction(frame, InstructionOutcome::Effect { actual })
    }
}
