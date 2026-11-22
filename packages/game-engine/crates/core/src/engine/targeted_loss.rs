//! CR 104.3e, 104.5, 608.2b, 800.4a. An activated effect causes immediate
//! loss, independently of the state-based loss conditions.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

impl Game {
    pub(crate) fn targeted_loss_authority(&self) -> Result<(BatchCause, PlayerId), String> {
        let Frame::Resolve { frame } = &self.state.rules.frame else {
            return Err("targeted loss requires a resolving activation".into());
        };
        self.validate_resolving(frame)?;
        let program = self.program_for(self.entry(frame.stack_id)?)?;
        let [player] = self.entry(frame.stack_id)?.player_targets.as_slice() else {
            return Err("targeted loss requires one announced player".into());
        };
        match &self.entry(frame.stack_id)?.kind {
            StackKind::Activated {
                ability: 0,
                source_handles,
            } => {
                let definition = self.copiable_definition(self.entry(frame.stack_id)?.source)?;
                if !definition
                    .activated
                    .first()
                    .is_some_and(artifact_loss_activation)
                    || self.current(self.entry(frame.stack_id)?.source).is_ok()
                {
                    return Err("targeted loss requires its paid sacrificed artifact".into());
                }
                self.validate_activated_entry(self.entry(frame.stack_id)?, 0, source_handles)?;
            }
            _ => return Err("unsupported targeted loss invocation".into()),
        };
        if frame.cursor != 0
            || frame.legal_targets != [true]
            || !matches!(
                program.instructions.as_slice(),
                [Instruction::LoseGameTargetPlayer {
                    target_slot: 0,
                    source_block: 1
                }]
            )
        {
            return Err("targeted loss instruction does not authorize departure".into());
        }
        Ok((self.instruction_cause(frame, 1)?, *player))
    }

    pub(super) fn resolve_targeted_loss(&mut self, frame: Resolving) -> Result<(), String> {
        let (cause, player) = self.targeted_loss_authority()?;
        if !matches!(&self.state.rules.frame, Frame::Resolve {frame: current} if *current == frame)
        {
            return Err("targeted loss resolving instance changed".into());
        }
        if !self.is_live(player) {
            return self.finish_ability(frame.stack_id, true);
        }
        let batch = self.commit_batch(
            cause,
            EventGroup::Occurrence {
                proposed: ProposedEvent::Lose {
                    player,
                    reason: PlayerLossReason::TargetedEffect,
                },
            },
        )?;
        if self.finish_committed_player_losses(&batch)? {
            return Ok(());
        }
        // A self-targeted activator can leave with the invocation. Completion
        // still uses the frozen resolving instance in that case.
        self.finish_ability(frame.stack_id, true)
    }
}
