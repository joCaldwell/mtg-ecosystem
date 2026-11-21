use super::*;
use mtg_contracts::ManaResolutionQuantity;

impl Game {
    pub(super) fn produce_resolution_mana(
        &mut self,
        frame: Resolving,
        color: Color,
        quantity: ManaResolutionQuantity,
        source_block: u32,
    ) -> Result<(), String> {
        let controller = self.entry(frame.stack_id)?.controller;
        let amount = match quantity {
            ManaResolutionQuantity::Fixed { amount } => amount,
            ManaResolutionQuantity::PerMatchingPermanent { mut amount, filter } => {
                let count = self
                    .state
                    .rules
                    .objects
                    .values()
                    .filter(|object| self.matches_permanent_filter(object.id, controller, &filter))
                    .count();
                amount.multiply(
                    &i64::try_from(count)
                        .map_err(|_| "permanent count overflow")?
                        .into(),
                );
                amount
            }
        };
        let actual = if amount == Quantity::zero() || !self.is_live(controller) {
            None
        } else {
            Some(self.commit(
                ProposedEvent::AddMana {
                    player: controller,
                    color,
                    amount,
                    restriction: None,
                    source: self.entry(frame.stack_id)?.source,
                },
                BatchCause::Instruction {
                    stack_id: frame.stack_id,
                    player: controller,
                    source: self.entry(frame.stack_id)?.source,
                    cursor: frame.cursor,
                    source_block,
                },
            )?)
        };
        self.finish_instruction(frame, InstructionOutcome::Effect { actual })
    }
}
