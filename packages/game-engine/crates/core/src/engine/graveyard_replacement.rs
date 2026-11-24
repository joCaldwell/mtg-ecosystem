//! Library-to-graveyard instruction completion and the graveyard replacement
//! wave's frame dispatch. CR 614.4–6, 616.1f, 701.24c/f/g (2026-09-25).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

impl RulesTxn {
    pub(super) fn complete_library_effect(
        &mut self,
        frame: Resolving,
        proposed: ProposedEvent,
        source_block: u32,
    ) -> Result<(), String> {
        let actual = self.commit(proposed, self.instruction_cause(&frame, source_block)?)?;
        self.finish_instruction(
            frame,
            InstructionOutcome::Effect {
                actual: Some(actual),
            },
        )
    }

    pub(super) fn replacement_request(&self) -> Option<Request> {
        match &self.state.rules.frame {
            Frame::GraveyardGroup { .. } => self.group_replacement_request(),
            _ => None,
        }
    }

    pub(crate) fn replacement_reveals(&self, object: ObjectId) -> bool {
        matches!(&self.state.rules.frame, Frame::GraveyardGroup { tx } if tx.reveals(object))
    }

    // Only the matching complete group and cause obtain this ephemeral proof.
    pub(super) fn prepare_graveyard_batch(
        &self,
        cause: &BatchCause,
        group: &EventGroup,
    ) -> Result<Option<BTreeMap<ObjectId, Zone>>, String> {
        if let Frame::GraveyardGroup { tx } = &self.state.rules.frame {
            return tx.authorize(self, cause, group);
        }
        Ok(None)
    }

    pub(super) fn replacement_charge(&self) -> usize {
        let frame = frame::retained_effect_frame(&self.state.rules.frames, &self.state.rules.frame);
        if matches!(frame, Frame::GraveyardGroup { .. }) {
            stored_history::payload_charge(frame)
        } else {
            0
        }
    }

    pub(super) fn validate_graveyard_replacement(&self) -> Result<(), String> {
        if let Frame::GraveyardGroup { tx } =
            frame::retained_effect_frame(&self.state.rules.frames, &self.state.rules.frame)
        {
            return tx.validate(self);
        }
        Ok(())
    }
}
