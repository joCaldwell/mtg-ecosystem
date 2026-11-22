//! CR 305.1–3 and 611.3: evaluate current battlefield permission sources.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

impl RulesTxn {
    pub(super) fn persistent_land_allowances(
        &self,
        player: PlayerId,
    ) -> impl Iterator<Item = ObjectId> {
        self.land_rule_sources(player, StaticEffect::AdditionalLandPlay)
    }

    pub(super) fn can_play_from_land_zone(&self, player: PlayerId, zone: Zone) -> bool {
        zone == (Zone::Hand { owner: player })
            || (zone == (Zone::Graveyard { owner: player })
                && self
                    .land_rule_sources(player, StaticEffect::PlayLandsFromOwnGraveyard)
                    .next()
                    .is_some())
    }

    fn land_rule_sources(
        &self,
        player: PlayerId,
        effect: StaticEffect,
    ) -> impl Iterator<Item = ObjectId> {
        self.catalog_has_land_reclamation
            .then_some(())
            .into_iter()
            .flat_map(|_| self.state.rules.objects.values())
            .filter(move |object| object.present_on_battlefield() && object.controller == player)
            .filter_map(move |object| {
                self.copiable_definition(object.id)
                    .is_ok_and(|definition| {
                        definition
                            .static_abilities
                            .iter()
                            .any(|ability| ability.effect == effect)
                    })
                    .then_some(object.id)
            })
    }
}
