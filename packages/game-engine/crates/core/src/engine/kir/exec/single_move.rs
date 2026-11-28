//! The movements outside the evaluated zone change: a resolution choice's
//! object to hand and the resolving spell's own shuffle (CR 400.3, 608.2n).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::super::*;

impl RulesTxn {
    /// CR 400.3, 608.2c, 903.9b: the object a resolution choice selected
    /// returns to its owner's hand; a commander's owner may choose the
    /// command zone instead.
    pub(in crate::engine) fn return_chosen_to_owner_hand(
        &mut self,
        frame: Resolving,
        target: ObjectId,
        source_block: u32,
    ) -> Result<(), String> {
        let owner = self.state.rules.cards[&target.card].owner;
        let destination = Zone::Hand { owner };
        if self.state.rules.commanders.history(target.card).is_some() {
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
        let movement = self.move_instruction_object(target, destination, cause)?;
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

    /// CR 608.2n, 701.24c: move the resolving spell itself, applying its
    /// stack-departure replacement if any, then shuffle and finish resolution.
    pub(in crate::engine) fn shuffle_resolving_spell(
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
