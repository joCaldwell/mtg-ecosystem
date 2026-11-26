//! CR 701.27, 712.9-712.10, 712.18: a permanent's own ability transforms it.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::super::*;

impl RulesTxn {
    /// Whether the invocation's source can transform now: it is still the
    /// same permanent on the battlefield, a transforming double-faced card,
    /// has not transformed since this ability was put onto the stack
    /// (CR 701.27f), and would not turn an instant or sorcery face up
    /// (CR 701.27c-d, 712.9-712.10).
    pub(in crate::engine) fn source_transformable(
        &self,
        entry: &StackInstance,
    ) -> Result<bool, String> {
        if entry.kind.is_spell() {
            return Err("a spell has no transforming source".into());
        }
        let Ok(object) = self.current(entry.source) else {
            return Ok(false);
        };
        let definition = self.definition(entry.source.card)?;
        Ok(object.present_on_battlefield()
            && definition.transforms()
            && object.transforms == entry.source_transforms
            && definition.face(object.face().other()).is_some_and(|face| {
                !matches!(
                    face.characteristics.kind,
                    CardKind::Instant | CardKind::Sorcery
                )
            }))
    }

    pub(in crate::engine) fn kernel_transform_source(
        &mut self,
        frame: Resolving,
        source_block: u32,
    ) -> Result<(), String> {
        let entry = self.entry(frame.stack_id)?;
        if self.source_transformable(entry)? {
            let object = entry.source;
            self.commit_batch(
                self.instruction_cause(&frame, source_block)?,
                EventGroup::Occurrence {
                    proposed: ProposedEvent::Transform { object },
                },
            )?;
        }
        self.finish_instruction(frame, InstructionOutcome::Effect { actual: None })
    }
}
