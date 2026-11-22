//! CR 106.3, 302.6, 605.3b, 608.2d/h and 733.1 (2026-09-25).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

impl RulesTxn {
    pub(super) fn single_color_mana_units(
        &self,
        player: PlayerId,
        color: Option<Color>,
        amount: &SingleColorManaAmount,
    ) -> Result<Vec<(Color, Quantity)>, String> {
        let color = color.ok_or("missing single-color mana choice")?;
        let quantity =
            match amount {
                SingleColorManaAmount::Fixed { quantity } => quantity.clone(),
                SingleColorManaAmount::ControllerEnchantments => {
                    // Count derived battlefield types at resolution, including
                    // the source if still controlled. A phased-out object is absent.
                    let mut count = Quantity::zero();
                    for object in self.state.rules.objects.values().filter(|object| {
                        object.present_on_battlefield() && object.controller == player
                    }) {
                        if self
                            .characteristics(object.id)?
                            .has_type(CardKind::Enchantment)
                        {
                            count.add(&1.into());
                        }
                    }
                    count
                }
            };
        // One exact production event and receipt for the chosen color.
        Ok(if quantity > Quantity::zero() {
            vec![(color, quantity)]
        } else {
            vec![]
        })
    }
}
