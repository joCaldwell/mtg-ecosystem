//! CR 121.2c, 704.4 and 800.4. Each player's draws form one part of the
//! resolving instruction, even after an empty-library draw.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlayerDrawAttempts {
    pub player: PlayerId,
    pub attempted: Quantity,
    pub failed: Quantity,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EachPlayerDrawProgress {
    pub(super) completed: Vec<PlayerDrawAttempts>,
    // The first remaining player owns the ordinary DrawProgress, if present.
    pub(super) remaining: Vec<PlayerId>,
}

impl Game {
    pub(super) fn finish_player_draws(
        &mut self,
        mut frame: Resolving,
        player: PlayerId,
        attempted: Quantity,
        failed: Quantity,
    ) -> Result<(), String> {
        let Some(scope) = &mut frame.each_player_draw else {
            return self.finish_instruction(
                frame,
                InstructionOutcome::DrawAttempts {
                    player,
                    attempted,
                    failed,
                },
            );
        };
        if scope.remaining.first() != Some(&player) {
            return Err("each-player draw actor invariant".into());
        }
        scope.remaining.remove(0);
        scope.completed.push(PlayerDrawAttempts {
            player,
            attempted,
            failed,
        });
        if scope.remaining.is_empty() {
            let players = std::mem::take(&mut scope.completed);
            frame.each_player_draw = None;
            self.finish_instruction(
                frame,
                InstructionOutcome::EachPlayerDrawAttempts { players },
            )
        } else {
            self.state.rules.frame = Frame::Resolve { frame };
            Ok(())
        }
    }

    pub(super) fn validate_each_player_draws(
        &self,
        completed: &[PlayerDrawAttempts],
        remaining: &[PlayerId],
        amount: &Quantity,
    ) -> Result<(), String> {
        if completed.len() > 4 || remaining.len() > 4 - completed.len() {
            return Err("each-player draw seat count invariant".into());
        }
        let frozen = completed
            .iter()
            .map(|row| row.player)
            .chain(remaining.iter().copied());
        // A departed seat can still be in the frozen order. Accepted replay
        // establishes which seats were live when the instruction started.
        let expected = self
            .apnap_seats()
            .filter(|seat| self.is_live(*seat) || frozen.clone().any(|player| player == *seat));
        if !frozen.clone().eq(expected) {
            return Err("each-player draw seat order invariant".into());
        }
        for row in completed {
            if row.failed.is_negative()
                || row.failed > row.attempted
                || row.attempted > *amount
                || (row.attempted != *amount && self.is_live(row.player))
            {
                return Err("each-player draw completed attempts invariant".into());
            }
        }
        Ok(())
    }
}
