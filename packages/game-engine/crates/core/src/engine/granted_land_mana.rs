//! CR 113.10, 302.6, 605.3, 611.3: the recipient land activates the
//! granted ability and is the exact source in the cost and mana receipt.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

impl RulesTxn {
    pub(crate) fn granted_land_mana_colors() -> [Color; 5] {
        [Color::W, Color::U, Color::B, Color::R, Color::G]
    }

    pub(crate) fn has_granted_land_mana(&self, land: ObjectId) -> Result<bool, String> {
        let current = self.current(land)?;
        if !current.present_on_battlefield()
            || !self.is_live(current.controller)
            || !self
                .copiable_definition(land)?
                .characteristics
                .has_type(CardKind::Land)
            || !self.prepared.catalog.definitions.iter().any(|definition| {
                definition
                    .capabilities
                    .contains(&Capability::ControllerLandGrantedManaV1)
            })
        {
            return Ok(false);
        }
        for source in self.state.rules.objects.values() {
            if source.present_on_battlefield()
                && source.controller == current.controller
                && self.is_live(self.state.rules.cards[&source.id.card].owner)
                && self
                    .copiable_definition(source.id)?
                    .static_abilities
                    .iter()
                    .any(|ability| {
                        ability.functioning_zone == FunctioningZone::Battlefield
                            && ability.effect == StaticEffect::GrantControllerLandsAnyColorMana
                    })
            {
                return Ok(true);
            }
        }
        Ok(false)
    }
}
