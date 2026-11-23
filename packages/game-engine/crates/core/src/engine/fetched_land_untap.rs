//! CR 400.7j, 608.2c and 701.26b (2026-09-25).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use std::num::NonZeroU32;

impl RulesTxn {
    pub(super) fn conditional_basic_fetch_minimum(
        &self,
        frame: &Resolving,
    ) -> Result<Option<NonZeroU32>, String> {
        let instruction = self
            .program_for(self.entry(frame.stack_id)?)?
            .instructions
            .get(frame.cursor as usize)
            .ok_or("missing shuffle instruction")?;
        Ok(match instruction {
            _ => None,
        })
    }

    pub(super) fn valid_fetched_land_movement(
        &self,
        player: PlayerId,
        movement: &MoveOutcome,
    ) -> bool {
        movement.from == (Zone::Library { owner: player })
            && movement.destination == Zone::Battlefield
            && movement.previous.card == movement.successor.card
            && movement.previous.incarnation.checked_add(1) == Some(movement.successor.incarnation)
            && self
                .state
                .rules
                .cards
                .get(&movement.successor.card)
                .is_some_and(|card| card.owner == player)
            && self
                .definition(movement.successor.card)
                .is_ok_and(|definition| {
                    definition.characteristics.basic
                        && definition.characteristics.has_type(CardKind::Land)
                })
    }
}
