//! Life gain and loss (CR 119).
use super::super::*;
use mtg_contracts::kir::{Amount, PlayerExpr};

impl RulesTxn {
    // CR 119.3, 608.2b-c, 704.4: one life event per instruction, with no
    // state-based action check until the program finishes. A self-targeted
    // loss may reach zero and recover before the next priority checkpoint.
    pub(in crate::engine) fn kernel_life(
        &mut self,
        frame: Resolving,
        gain: bool,
        player: &PlayerExpr,
        amount: &Amount,
        source_block: u32,
    ) -> Result<(), String> {
        let skipped = |game: &mut Self, frame| {
            game.finish_instruction(frame, InstructionOutcome::Life { actual: None })
        };
        let player = self.kernel_players(&frame, player)?;
        if matches!(player, Players::Nobody) {
            return skipped(self, frame);
        }
        let amount = self.kernel_amount(&frame, amount)?;
        // CR 119.8: changing life by zero produces no event.
        if amount == Quantity::zero() {
            return skipped(self, frame);
        }
        let player = match player {
            Players::EachOpponent => {
                self.commit_opponent_life(&frame, &amount, source_block, gain)?;
                return skipped(self, frame);
            }
            Players::Nobody => return skipped(self, frame),
            Players::One(player) => player,
        };
        if !self.is_live(player) {
            return skipped(self, frame);
        }
        let proposed = if gain {
            ProposedEvent::GainLife { player, amount }
        } else {
            ProposedEvent::LoseLife { player, amount }
        };
        let actual = self.commit(proposed, self.instruction_cause(&frame, source_block)?)?;
        self.finish_instruction(
            frame,
            InstructionOutcome::Life {
                actual: Some(actual),
            },
        )
    }
}
