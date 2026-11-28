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

    /// CR 115.1, 603.2, 702.21a: the stack object whose announcement made a
    /// permanent this trigger's target, its counter domain and the player
    /// who controlled it then.
    pub(super) fn event_stack_object(
        &self,
        stack_id: StackId,
    ) -> Result<(StackId, TargetDomain, PlayerId), String> {
        let StackKind::Triggered {
            event: TriggerOccurrenceId::Target(occurrence),
            witness:
                TriggerEventWitness::BecameTarget {
                    actor, is_spell, ..
                },
            ..
        } = &self.entry(stack_id)?.kind
        else {
            return Err("event stack object requires a targeting trigger".into());
        };
        let domain = if *is_spell {
            TargetDomain::StackSpell
        } else {
            TargetDomain::StackActivatedOrTriggeredAbility
        };
        Ok((occurrence.targeting, domain, *actor))
    }

    /// CR 109.4, 702.21a: "that player" is the current controller of the
    /// event's stack object, or its controller as it targeted once it has
    /// left the stack.
    pub(super) fn event_stack_controller(&self, stack_id: StackId) -> Result<PlayerId, String> {
        let (target, _, actor) = self.event_stack_object(stack_id)?;
        Ok(self
            .state
            .rules
            .stack
            .iter()
            .find(|entry| entry.id == target)
            .map_or(actor, |entry| entry.controller))
    }

    /// The stack object a counter instruction names, its domain and whether
    /// it is still a legal choice (CR 608.2b): an announced target slot or a
    /// targeting trigger's event stack object, which is never illegal.
    fn counter_binding(
        &self,
        frame: &Resolving,
        what: Ref,
    ) -> Result<(StackId, TargetDomain, bool), String> {
        match what {
            Ref::Target { slot } => {
                let (target, domain) = self.counter_target(frame, slot)?;
                Ok((
                    target,
                    domain,
                    frame.legal_targets.get(slot as usize) == Some(&true),
                ))
            }
            Ref::EventStackObject => {
                let (target, domain, _) = self.event_stack_object(frame.stack_id)?;
                Ok((target, domain, true))
            }
            _ => Err("counter requires an announced or event stack object".into()),
        }
    }

    pub(super) fn execute_counter(
        &mut self,
        frame: Resolving,
        what: Ref,
        source_block: u32,
    ) -> Result<(), String> {
        let (target, domain, legal) = self.counter_binding(&frame, what)?;
        let outcome = if !legal {
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
        let Some(kernel!(Counter { what } @ block)) = instruction else {
            return Err("counter requires a counter instruction".into());
        };
        if *countering != frame.stack_id
            || self.counter_binding(frame, *what)? != (*target, *domain, true)
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
                let Some((first, last)) = events.first().zip(events.last()) else {
                    return Ok(());
                };
                let batch = first.occurrence.batch;
                let simultaneous = first.occurrence != last.occurrence;
                // Proliferate may put multiple counter kinds on one recipient.
                // Each recipient has one occurrence and consecutive members.
                let occurrences = events.chunk_by(|a, b| a.occurrence == b.occurrence);
                if batch >= self.state.rules.next_batch_id
                    || occurrences.enumerate().any(|(index, members)| {
                        let occurrence = &members[0].occurrence;
                        index >= self.event_group_limit()
                            || occurrence.batch != batch
                            || if simultaneous {
                                occurrence.path != [index as u32]
                            } else {
                                !occurrence.path.is_empty()
                            }
                            || members
                                .iter()
                                .enumerate()
                                .any(|(member, event)| usize::try_from(event.member) != Ok(member))
                    })
                {
                    return Err("snapshot counter placement event invariant".into());
                }
            }
            InstructionOutcome::Countered { target, movement } => {
                // A targeting trigger announces no target and counters its
                // event's stack object (CR 702.21a).
                let entry = self.entry(frame.stack_id)?;
                let what = if self.original_program_for(entry)?.targets.is_empty()
                    && self.event_stack_object(frame.stack_id).is_ok()
                {
                    Ref::EventStackObject
                } else {
                    Ref::Target { slot: 0 }
                };
                let (bound, domain, legal) = self.counter_binding(frame, what)?;
                if *target != bound
                    || !legal
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
