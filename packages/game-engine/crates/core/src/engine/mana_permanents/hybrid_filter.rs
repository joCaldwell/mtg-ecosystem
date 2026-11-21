//! CR 107.4e, 601.2b, 602.2b, 605.3b and 733.1 (2026-09-25).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

pub(super) fn uses_indexed_paid_mana(definition: &Definition) -> bool {
    hybrid_filter_colors(definition).is_some()
        || mtg_contracts::paid_restricted_land::present(definition)
}

impl ManaActivation {
    pub(super) fn filter_colors(&self) -> Option<[Color; 2]> {
        match self.ability.production {
            ManaProduction::HybridFilter { colors } => Some(colors),
            _ => None,
        }
    }

    pub(super) fn payment_cost(&self) -> Option<ManaCost> {
        if self.filter_colors().is_some() {
            return self.filter_input.map(|color| ManaCost {
                colored: vec![color],
                ..zero_mana_cost()
            });
        }
        self.ability.costs.mana.clone()
    }

    pub(super) fn filter_input_colors(&self) -> Result<Vec<Color>, String> {
        self.filter_colors()
            .map(|colors| colors.to_vec())
            .ok_or_else(|| "missing hybrid filter input".into())
    }

    pub(super) fn validate_filter_stage(&self, definition: &Definition) -> bool {
        if let Some(colors) = self.filter_colors() {
            let input_valid = if self.stage == ManaStage::FilterInput {
                self.filter_input.is_none()
            } else {
                self.filter_input
                    .is_some_and(|color| colors.contains(&color))
            };
            return hybrid_filter_colors(definition) == Some(colors)
                && self.printed_index == Some(1)
                && definition.mana_abilities.get(1) == Some(&self.ability)
                && input_valid
                && match &self.stage {
                    ManaStage::FilterInput | ManaStage::Payment | ManaStage::Spend => true,
                    ManaStage::Combination { total } => *total == Quantity::from(2),
                    ManaStage::Color => false,
                };
        }
        if mtg_contracts::paid_restricted_land::present(definition) {
            return self.filter_input.is_none()
                && self.printed_index == Some(1)
                && definition.mana_abilities.get(1) == Some(&self.ability)
                && matches!(self.stage, ManaStage::Payment | ManaStage::Spend);
        }
        self.filter_input.is_none()
            && self.printed_index.is_none()
            && self.stage != ManaStage::FilterInput
            && definition.mana_abilities.as_slice() == std::slice::from_ref(&self.ability)
    }
}
