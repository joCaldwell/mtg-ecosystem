//! Accepting a validated answer and advancing the game.
use crate::engine::*;

impl Game {
    /// Submit is a bounded state transition; automatic rules run in advance.
    pub fn submit(&mut self, receipt: ValidatedResponse) -> Result<(), String> {
        Ok(self.accept(receipt)?)
    }
    pub fn respond(
        &mut self,
        player: PlayerId,
        decision_id: u64,
        response: Answer,
    ) -> Result<(), String> {
        let r = self.check_response(player, decision_id, response)?;
        self.submit(r)
    }
    pub fn advance(&mut self, budget: Budget) -> Advance {
        self.advance_internal::<false, true>(budget, &mut vec![])
    }
}
