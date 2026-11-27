//! CR 301.5, 701.3b, 702.6a: an unchanged host keeps its timestamp.
use super::super::*;
impl RulesTxn {
    pub(in crate::engine) fn kernel_equip(
        &mut self,
        frame: Resolving,
        source_block: u32,
    ) -> Result<(), String> {
        let entry = self.entry(frame.stack_id)?;
        let source = entry.source;
        let target = *entry.targets.first().ok_or("missing equip target")?;
        let actual = if frame.legal_targets.first() == Some(&true)
            && self
                .current(source)
                .is_ok_and(|o| o.present_on_battlefield() && o.attached_to != Some(target))
            && source != target
            && self.attachment_target_type_valid(source, target)?
            && !self.protected_from(target, self.color_layer(source)?.as_ref())?
        {
            Some(self.commit(
                ProposedEvent::Attach {
                    object: source,
                    target: Some(target),
                },
                self.instruction_cause(&frame, source_block)?,
            )?)
        } else {
            None
        };
        self.finish_instruction(frame, InstructionOutcome::Effect { actual })
    }
}
