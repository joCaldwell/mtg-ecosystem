//! One announced target or source incarnation moves (CR 400.3, 400.7, 608.2b).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::super::*;
use mtg_contracts::kir::{Destination, Ref};

impl RulesTxn {
    pub(in crate::engine) fn kernel_single_move(
        &mut self,
        frame: Resolving,
        object: Ref,
        to: Destination,
        source_block: u32,
    ) -> Result<(), String> {
        let entry = self.entry(frame.stack_id)?;
        let (target, domain) = match object {
            Ref::Target { slot } => {
                let slot = slot as usize;
                let target = *self
                    .bound_object_target(entry, slot)
                    .ok_or("missing move target")?;
                if frame.legal_targets.get(slot) != Some(&true) {
                    return self
                        .finish_instruction(frame, InstructionOutcome::IllegalTarget { target });
                }
                (target, Some(self.program_for(entry)?.targets[slot].domain))
            }
            Ref::Source => (entry.source, None),
            _ => return Err("zone move requires a source or target".into()),
        };
        let actor = entry.controller;
        if !self.current(target).is_ok_and(|object| {
            if let Some(
                domain @ (TargetDomain::OwnGraveyardCard
                | TargetDomain::OwnGraveyardCreatureCard
                | TargetDomain::GraveyardCard { .. }),
            ) = domain
            {
                self.graveyard_target_legal(object, actor, domain)
            } else {
                object.present_on_battlefield()
            }
        }) {
            return self
                .finish_instruction(frame, InstructionOutcome::ObjectNoLongerPresent { target });
        }
        // CR 303.4f-g, 614.12: a noncreature target can be an Aura or
        // have an as-enters choice. Use the shared card-entry procedure.
        if matches!(to, Destination::Battlefield { .. })
            && let Ref::Target { slot } = object
            && self.program_for(entry)?.targets[slot as usize].domain
                == TargetDomain::OwnGraveyardCard
        {
            return self.continue_card_entry(
                frame,
                actor,
                vec![target],
                vec![],
                vec![],
                vec![],
                source_block,
            );
        }
        self.kernel_move_present(frame, target, to, source_block)
    }

    pub(in crate::engine) fn kernel_move_present(
        &mut self,
        frame: Resolving,
        target: ObjectId,
        to: Destination,
        source_block: u32,
    ) -> Result<(), String> {
        let owner = self.state.rules.cards[&target.card].owner;
        let destination = match to {
            Destination::OwnerHand => Zone::Hand { owner },
            Destination::OwnerLibraryTop => Zone::Library { owner },
            // CR 110.2a, 614.1c: the target is in the invocation controller's
            // own graveyard. Its owner controls it there and on entry.
            // Admission restricts this move to creatures, excluding Auras
            // and the land-only entry life payment cell.
            Destination::Battlefield { .. } => Zone::Battlefield,
            _ => return Err("unsupported destination for a single move".into()),
        };
        if destination != Zone::Battlefield
            && self.state.rules.commanders.history(target.card).is_some()
        {
            self.rules_mut().frame = Frame::CommanderReturn {
                object: target,
                destination,
                continuation: CommanderContinuation::SpellMove {
                    frame,
                    source_block,
                },
            };
            return Ok(());
        }
        let cause = self.instruction_cause(&frame, source_block)?;
        let movement = if matches!(to, Destination::Battlefield { tapped: true, .. }) {
            let batch = self.commit_entry_batch(
                cause,
                EventGroup::Occurrence {
                    proposed: ProposedEvent::Move {
                        object: target,
                        destination,
                    },
                },
                &[(target, events::EntryOverride::ForceTapped)],
            )?;
            let Some(EventOutcome::Moved { movement }) = batch.result(&[]) else {
                return Err("missing tapped entry outcome".into());
            };
            movement.clone()
        } else {
            self.move_instruction_object(target, destination, cause)?
        };
        self.finish_instruction(frame, InstructionOutcome::Moved { movement })
    }
}
