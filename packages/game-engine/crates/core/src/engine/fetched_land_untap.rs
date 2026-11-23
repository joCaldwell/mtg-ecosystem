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

    pub(super) fn searched_basic_successor(
        &self,
        player: PlayerId,
        batch: &CommittedBatch,
    ) -> Result<MoveOutcome, String> {
        let mut searched = batch.occurrences.iter().filter_map(|occurrence| {
            match (&occurrence.proposed, &occurrence.actual) {
                _ => None,
            }
        });
        let movement = searched.next().ok_or("missing searched basic land entry")?;
        if searched.next().is_some() || !self.valid_fetched_land_movement(player, movement) {
            return Err("invalid searched basic land entry".into());
        }
        Ok(movement.clone())
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

    pub(super) fn finish_basic_fetch_untap(
        &mut self,
        frame: &Resolving,
        source_block: u32,
        movement: Option<MoveOutcome>,
    ) -> Result<Option<EventOutcome>, String> {
        let Some(minimum) = self.conditional_basic_fetch_minimum(frame)? else {
            return Ok(None);
        };
        let Some(movement) = movement else {
            return Ok(None);
        };
        let object = movement.successor;
        if !self
            .current(object)
            .is_ok_and(|land| land.present_on_battlefield() && land.tapped)
        {
            return Ok(None);
        }
        let player = self.entry(frame.stack_id)?.controller;
        let mut count = 0;
        for land in self.state.rules.objects.values() {
            if land.present_on_battlefield()
                && land.controller == player
                && self.characteristics(land.id)?.has_type(CardKind::Land)
            {
                count += 1;
                if count == minimum.get() {
                    let cause = self.instruction_cause(frame, source_block)?;
                    return self
                        .commit(ProposedEvent::Untap { object }, cause)
                        .map(Some);
                }
            }
        }
        Ok(None)
    }
}
