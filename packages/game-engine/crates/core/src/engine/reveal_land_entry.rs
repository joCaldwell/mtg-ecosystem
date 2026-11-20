//! CR 109.5, 205.3i, 614.12a, 616.1c and 701.20a–c (2026-09-25).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

impl Game {
    pub(super) fn entry_reveal_eligible(
        &self,
        player: PlayerId,
        card: ObjectId,
        subtypes: [BasicLandType; 2],
    ) -> bool {
        self.zone(card) == Some(Zone::Hand { owner: player })
            && self.definition(card.card).is_ok_and(|d| {
                d.characteristics.has_type(CardKind::Land)
                    && d.characteristics
                        .subtypes
                        .iter()
                        .any(|name| subtypes.iter().any(|kind| kind.subtype() == name))
            })
    }

    pub(super) fn entry_reveal_candidates(
        &self,
        player: PlayerId,
        subtypes: [BasicLandType; 2],
    ) -> Vec<ObjectId> {
        self.state
            .rules
            .objects
            .values()
            .filter(|card| self.entry_reveal_eligible(player, card.id, subtypes))
            .map(|card| card.id)
            .collect()
    }

    pub(super) fn apply_entry_hand_reveal(
        &mut self,
        player: PlayerId,
        card: ObjectId,
        authority: Option<&PreparedEntryAuthority>,
    ) -> Result<EventOutcome, String> {
        if authority.is_none_or(|proof| !proof.reveals(player, card))
            || self.zone(card) != Some(Zone::Hand { owner: player })
        {
            return Err("hand reveal requires the exact prepared entry choice".into());
        }
        for knowledge in &mut self.state.knowledge.visibility {
            knowledge.revealed_hand.insert(card);
        }
        Ok(EventOutcome::HandCardRevealedForEntry { player, card })
    }
}
