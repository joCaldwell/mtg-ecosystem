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
        if object == Ref::Source && to == Destination::OwnerLibraryShuffled {
            return self.shuffle_resolving_spell(frame, source_block);
        }
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
            if to == Destination::OwnerLibraryShuffled {
                self.shuffle_absent_object_owner(&frame, target, source_block)?;
            }
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
            Destination::OwnerLibraryTop
            | Destination::OwnerLibraryBottom
            | Destination::OwnerLibraryShuffled => Zone::Library { owner },
            // CR 110.2a, 614.1c: creature entry can specify a controller
            // and tapped status. Aura and land entry use the choice path.
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
        let movement = if let Destination::Battlefield { controller, tapped } = &to {
            if !controller.is_you() {
                return Err("battlefield move requires an audited controller".into());
            }
            let player = self.entry(frame.stack_id)?.controller;
            if !self.is_live(player) {
                return self
                    .finish_instruction(frame, InstructionOutcome::PlayerActionSkipped { player });
            }
            let entry_override = if player != owner {
                Some(events::EntryOverride::CreatureController {
                    player,
                    tapped: *tapped,
                })
            } else {
                tapped.then_some(events::EntryOverride::ForceTapped)
            };
            let Some(entry_override) = entry_override else {
                let movement = self.move_instruction_object(target, destination, cause)?;
                return self.finish_instruction(frame, InstructionOutcome::Moved { movement });
            };
            let batch = self.commit_entry_batch(
                cause,
                EventGroup::Occurrence {
                    proposed: ProposedEvent::Move {
                        object: target,
                        destination,
                    },
                },
                &[(target, entry_override)],
            )?;
            let Some(EventOutcome::Moved { movement }) = batch.result(&[]) else {
                return Err("missing tapped entry outcome".into());
            };
            movement.clone()
        } else if to == Destination::OwnerLibraryShuffled {
            self.move_and_shuffle_owner_library(target, destination, cause)?
        } else if to == Destination::OwnerLibraryBottom {
            // The ordinary Move appends to the bottom (CR 401.4).
            self.move_object_for(target, destination, cause)?
        } else {
            self.move_instruction_object(target, destination, cause)?
        };
        self.finish_instruction(frame, InstructionOutcome::Moved { movement })
    }

    /// CR 701.24c: the movement and shuffle are ordered in one transaction.
    /// The object's owner shuffles even if a replacement diverted the move.
    pub(in crate::engine) fn move_and_shuffle_owner_library(
        &mut self,
        object: ObjectId,
        destination: Zone,
        cause: BatchCause,
    ) -> Result<MoveOutcome, String> {
        let player = self.state.rules.cards[&object.card].owner;
        let batch = self.commit_batch(
            cause,
            EventGroup::Ordered {
                children: vec![
                    EventGroup::Occurrence {
                        proposed: ProposedEvent::Move {
                            object,
                            destination,
                        },
                    },
                    EventGroup::Occurrence {
                        proposed: ProposedEvent::ShuffleLibrary { player },
                    },
                ],
            },
        )?;
        let Some(EventOutcome::Moved { movement }) = batch.result(&[0]) else {
            return Err("missing movement before library shuffle".into());
        };
        Ok(movement.clone())
    }

    pub(in crate::engine) fn shuffle_absent_object_owner(
        &mut self,
        frame: &Resolving,
        object: ObjectId,
        source_block: u32,
    ) -> Result<(), String> {
        let player = self.state.rules.cards[&object.card].owner;
        if self.is_live(player) {
            self.commit(
                ProposedEvent::ShuffleLibrary { player },
                self.instruction_cause(frame, source_block)?,
            )?;
        }
        Ok(())
    }

    /// CR 608.2n, 701.24c: move the resolving spell itself, applying its
    /// stack-departure replacement if any, then shuffle and finish resolution.
    fn shuffle_resolving_spell(
        &mut self,
        frame: Resolving,
        source_block: u32,
    ) -> Result<(), String> {
        let entry = self.entry(frame.stack_id)?;
        let program = self.program_for(entry)?;
        if !entry.kind.is_spell() || frame.cursor as usize + 1 != program.instructions.len() {
            return Err("resolving spell shuffle is not its final instruction".into());
        }
        if let Some(index) = self.resolving_entry_index(entry.id)? {
            let object = entry.source;
            let owner = self.state.rules.cards[&object.card].owner;
            let destination = self.stack_departure(entry, Zone::Library { owner });
            self.move_and_shuffle_owner_library(
                object,
                destination,
                self.instruction_cause(&frame, source_block)?,
            )?;
            self.unstack(index).map_err(|e| e.to_string())?;
        }
        self.complete_spell_resolution(frame.stack_id, true)
            .map_err(|e| e.to_string())
    }
}
