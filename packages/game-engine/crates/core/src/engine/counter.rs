//! CR 608.2b, 701.6: counter the announced stack instance, leaving paid
//! costs and cast history intact. CR 702.34a still applies to stack departure.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use mtg_contracts::kir::Ref;

impl RulesTxn {
    pub(super) fn bind_stack_targets(&mut self, cast: Casting) -> Result<(), String> {
        let input = self
            .rules_mut()
            .pending
            .take()
            .ok_or("missing stack targets")?;
        let targets = input.stack_targets()?;
        let announced = if targets.is_empty() {
            vec![]
        } else {
            (0..4)
                .map(|viewer| {
                    targets
                        .iter()
                        .map(|&target| self.stack_handle_id(PlayerId(viewer), target).cloned())
                        .collect::<Result<Vec<_>, _>>()
                })
                .collect::<Result<Vec<_>, _>>()?
        };
        let top = self
            .rules_mut()
            .stack
            .last_mut()
            .ok_or("missing stack-targeting invocation")?;
        top.stack_targets = targets;
        top.announced_stack_targets = announced;
        if cast.origin.is_spell() {
            self.rules_mut().frame = self.announcement_mana_frame(cast)?;
        } else {
            self.continue_activation_after_targets(cast, input.decision_id)?;
        }
        Ok(())
    }

    fn counter_target(
        &self,
        frame: &Resolving,
        slot: u32,
    ) -> Result<(StackId, TargetDomain), String> {
        let entry = self.entry(frame.stack_id)?;
        let target = *self
            .bound_stack_target(entry, slot as usize)
            .ok_or("missing counter target")?;
        let spec = self
            .program_for(entry)?
            .targets
            .get(slot as usize)
            .ok_or("missing counter target spec")?;
        Ok((target, spec.domain))
    }

    pub(super) fn execute_counter(
        &mut self,
        frame: Resolving,
        what: Ref,
        source_block: u32,
    ) -> Result<(), String> {
        let Ref::Target { slot } = what else {
            return Err("counter requires an announced stack target".into());
        };
        let (target, domain) = self.counter_target(&frame, slot)?;
        let outcome = if frame.legal_targets.get(slot as usize) != Some(&true) {
            InstructionOutcome::StackTargetIllegal { target }
        } else if !self
            .state
            .rules
            .stack
            .iter()
            .any(|entry| entry.id == target)
        {
            InstructionOutcome::StackObjectNoLongerPresent { target }
        } else {
            match self.commit(
                ProposedEvent::CounterStackObject {
                    target,
                    countering: frame.stack_id,
                    domain,
                },
                self.instruction_cause(&frame, source_block)?,
            )? {
                EventOutcome::StackObjectCountered { movement, .. } => {
                    InstructionOutcome::Countered { target, movement }
                }
                EventOutcome::CounterPrevented { .. } => {
                    InstructionOutcome::CounterPrevented { target }
                }
                _ => return Err("missing counter outcome".into()),
            }
        };
        self.finish_instruction(frame, outcome)
    }

    pub(super) fn authorize_counter(
        &self,
        cause: &BatchCause,
        group: &EventGroup,
    ) -> Result<(), String> {
        let (
            Frame::Resolve { frame },
            EventGroup::Occurrence {
                proposed:
                    ProposedEvent::CounterStackObject {
                        target,
                        countering,
                        domain,
                    },
            },
        ) = (&self.state.rules.frame, group)
        else {
            return Err("counter requires a resolving instruction and single occurrence".into());
        };
        let instruction = self
            .program_for(self.entry(frame.stack_id)?)?
            .instructions
            .get(frame.cursor as usize);
        let Some(kernel!(Counter { what: Ref::Target { slot } } @ block)) = instruction else {
            return Err("counter requires a counter instruction".into());
        };
        if *countering != frame.stack_id
            || self.counter_target(frame, *slot)? != (*target, *domain)
            || frame.legal_targets.get(*slot as usize) != Some(&true)
            || *cause != self.instruction_cause(frame, *block)?
        {
            return Err("counter occurrence does not match resolution".into());
        }
        Ok(())
    }

    pub(super) fn validate_counter_outcome(
        &self,
        frame: &Resolving,
        outcome: &InstructionOutcome,
    ) -> Result<(), String> {
        match outcome {
            InstructionOutcome::Kernel { outcomes } => {
                for outcome in outcomes {
                    self.validate_counter_outcome(frame, outcome)?;
                }
            }
            InstructionOutcome::CounterPlacements { events } => {
                // One batch per placement step and one counter kind per
                // occurrence. Simultaneous recipients get consecutive paths.
                if let Some(first) = events.first()
                    && (first.occurrence.batch >= self.state.rules.next_batch_id
                        || events.len() > self.event_group_limit()
                        || events.iter().enumerate().any(|(index, event)| {
                            event.member != 0
                                || event.occurrence.batch != first.occurrence.batch
                                || if events.len() == 1 {
                                    !event.occurrence.path.is_empty()
                                } else {
                                    event.occurrence.path != [index as u32]
                                }
                        }))
                {
                    return Err("snapshot counter placement event invariant".into());
                }
            }
            InstructionOutcome::Countered { target, movement } => {
                let (bound, domain) = self.counter_target(frame, 0)?;
                if *target != bound
                    || frame.legal_targets.first() != Some(&true)
                    || self
                        .state
                        .rules
                        .stack
                        .iter()
                        .any(|entry| entry.id == *target)
                    || (movement.is_some()
                        == (domain == TargetDomain::StackActivatedOrTriggeredAbility))
                    || movement.as_ref().is_some_and(|m| {
                        m.from != Zone::Stack
                            || !matches!(m.destination, Zone::Graveyard { .. } | Zone::Exile)
                    })
                {
                    return Err("snapshot counter outcome invariant".into());
                }
            }
            _ => {}
        }
        Ok(())
    }
}
