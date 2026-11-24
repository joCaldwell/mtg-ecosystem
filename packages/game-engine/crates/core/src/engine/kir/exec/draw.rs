//! Draw (CR 121).
use super::super::*;
use mtg_contracts::kir::{Amount, PlayerExpr, Ref};

impl RulesTxn {
    // CR 121.1-121.2, 608.2b: one card by one player is a single draw whose
    // only recipient may be an illegal target or an absent event player.
    // Every other draw is the counted sequence of single draws.
    pub(in crate::engine) fn kernel_draw(
        &mut self,
        frame: Resolving,
        player: &PlayerExpr,
        amount: &Amount,
        source_block: u32,
    ) -> Result<(), String> {
        if !mtg_contracts::kir::single_draw(player, amount) {
            return self.draw_multiple(frame);
        }
        let skipped = |game: &mut Self, frame| {
            game.finish_instruction(frame, InstructionOutcome::Effect { actual: None })
        };
        let player = match player {
            PlayerExpr::Is {
                who: Ref::Target { slot },
            } => {
                let slot = *slot as usize;
                if frame.legal_targets.get(slot) != Some(&true) {
                    return skipped(self, frame);
                }
                *self
                    .bound_player_target(self.entry(frame.stack_id)?, slot)
                    .ok_or("missing draw player target")?
            }
            other => match self.kernel_players(&frame, other)? {
                Players::One(player) => player,
                Players::Nobody => return skipped(self, frame),
                Players::EachOpponent => return Err("unsupported each-opponent draw".into()),
            },
        };
        self.draw_one(frame, player, source_block)
    }
}
