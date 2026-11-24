//! CR 701.26, 608.2h.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::super::*;

impl RulesTxn {
    pub(in crate::engine) fn kernel_tap_set(
        &mut self,
        frame: Resolving,
        zone: &ZoneExpr,
        filter: &Pred,
        tap: bool,
        source_block: u32,
    ) -> Result<(), String> {
        let (objects, _) = self.kernel_set(&frame, zone, filter)?;
        let mut occurrences = Vec::new();
        for object in objects {
            let current = self.current(object)?;
            if current.present_on_battlefield() && current.tapped != tap {
                occurrences.push(if tap {
                    ProposedEvent::Tap { object }
                } else {
                    ProposedEvent::Untap { object }
                });
            }
        }
        if !occurrences.is_empty() {
            self.commit_batch(
                self.instruction_cause(&frame, source_block)?,
                EventGroup::Simultaneous { occurrences },
            )?;
        }
        self.finish_instruction(frame, InstructionOutcome::Effect { actual: None })
    }
}
