//! Keep unspent mana in black at the checked step/phase boundary.
//! CR 106.4, 500.5 and 614.1a:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

impl Game {
    pub(super) fn has_unspent_mana_converter(&self) -> bool {
        self.prepared.catalog.definitions.iter().any(|d| {
            d.capabilities
                .contains(&Capability::UnspentManaConversionV1)
        })
    }

    pub(super) fn unspent_mana_converters(&self, player: PlayerId) -> Vec<ObjectId> {
        if !self.has_unspent_mana_converter() {
            return vec![];
        }
        self.state
            .rules
            .objects
            .values()
            .filter(|o| {
                o.controller == player
                    && o.present_on_battlefield()
                    && self.is_live(self.state.rules.cards[&o.id.card].owner)
                    && self.definition(o.id.card).is_ok_and(|d| {
                        d.static_abilities.iter().any(|a| {
                            a.functioning_zone == FunctioningZone::Battlefield
                                && a.effect == StaticEffect::KeepUnspentManaAsBlack
                        })
                    })
            })
            .map(|o| o.id)
            .collect()
    }
}
