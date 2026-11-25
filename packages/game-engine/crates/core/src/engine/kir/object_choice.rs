//! CR 608.2c-d, 800.4a: the controller chooses from live public objects.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use mtg_contracts::kir::ObjectChoiceAction;

impl RulesTxn {
    pub(in crate::engine) fn object_choice_spec(
        &self,
        frame: &Resolving,
    ) -> Result<(&SetExpr, ObjectChoiceAction, u32), String> {
        let Some(Instruction::Kernel {
            node: Node::ChooseObject { from, action },
            source_block,
        }) = self
            .program_for(self.entry(frame.stack_id)?)?
            .instructions
            .get(frame.cursor as usize)
        else {
            return Err("missing resolution object choice instruction".into());
        };
        Ok((from, *action, *source_block))
    }

    pub(in crate::engine) fn object_choice_request(
        &self,
        frame: &Resolving,
    ) -> Result<Option<Decision<Rules>>, String> {
        let (from, _, _) = self.object_choice_spec(frame)?;
        let player = self.entry(frame.stack_id)?.controller;
        // CR 800.4a/g: once this controller leaves, no permanents can
        // still satisfy “you control”, so there is no choice to substitute.
        if !self.is_live(player) {
            return Ok(None);
        }
        let SetExpr::Matching { zone, filter } = from else {
            return Err("invalid resolution object choice set".into());
        };
        let (objects, _) = self.kernel_set(frame, zone, filter)?;
        Ok((!objects.is_empty())
            .then(|| self.pick_objects(player, Reason::ResolveObjectChoice, objects, 1)))
    }

    pub(in crate::engine) fn object_choice_step(&mut self, frame: Resolving) -> Result<(), String> {
        if self.object_choice_request(&frame)?.is_none() {
            self.rules_mut().pending = None;
            return self.finish_instruction(frame, InstructionOutcome::Effect { actual: None });
        }
        let pending = self
            .state
            .rules
            .pending
            .as_ref()
            .ok_or("missing object choice")?;
        self.validate_pending(pending)?;
        let object = pending.object()?;
        let (_, action, source_block) = self.object_choice_spec(&frame)?;
        self.rules_mut().pending = None;
        match action {
            ObjectChoiceAction::ReturnToOwnerHand => {
                self.kernel_move_present(frame, object, Destination::OwnerHand, source_block)
            }
        }
    }
}
