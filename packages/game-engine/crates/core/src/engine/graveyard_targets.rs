//! CR 601.2c and 608.2b: the caster selects a player and distinct physical
//! cards in that player's graveyard. Recheck the announced incarnations only.
use super::*;

impl RulesTxn {
    pub(crate) fn player_graveyard_card(&self, object: &Object, player: PlayerId) -> bool {
        object.zone == (Zone::Graveyard { owner: player })
            && !self.state.rules.tokens.contains_key(&object.id.card)
            && self
                .state
                .rules
                .cards
                .get(&object.id.card)
                .is_some_and(|c| c.owner == player)
    }

    pub(crate) fn remaining_graveyard_targets(
        &self,
        owner: PlayerId,
        cards: &[ObjectId],
    ) -> Vec<ObjectId> {
        cards
            .iter()
            .copied()
            .filter(|id| {
                self.current(*id)
                    .is_ok_and(|o| self.player_graveyard_card(o, owner))
            })
            .collect()
    }
}
