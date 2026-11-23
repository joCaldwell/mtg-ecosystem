//! CR 702.103a-e, 207.2a, 205.1a, 208.3. Definition supplies only the
//! printed characteristics; the casting procedure supplies the chosen form.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

/// Nonserialized operands for the supported single-faced enchantment creature.
/// The alternative changes what is paid, never the printed mana cost.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BestowCastPlan {
    printed: Characteristics,
    bestow_cost: ManaCost,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BestowForm {
    Creature,
    Aura,
}

impl BestowCastPlan {
    pub fn new(printed: &Characteristics, bestow_cost: ManaCost) -> Result<Self, String> {
        if printed.kind != CardKind::Creature
            || printed.additional_types != [CardKind::Enchantment]
            || printed.basic
            || printed.power.is_none()
            || printed.toughness.is_none()
            || printed
                .subtypes
                .iter()
                .any(|s| unsupported_rules_subtype(s) || s == "Aura")
            || printed.mana_cost.generic.is_negative()
            || bestow_cost.generic.is_negative()
        {
            return Err(
                "bestow requires a fixed-cost enchantment creature with ordinary creature subtypes"
                    .into(),
            );
        }
        Ok(Self {
            printed: printed.clone(),
            bestow_cost,
        })
    }
    pub fn base_cost(&self, form: BestowForm) -> &ManaCost {
        match form {
            BestowForm::Creature => &self.printed.mana_cost,
            BestowForm::Aura => &self.bestow_cost,
        }
    }
    pub fn target_domain(&self, form: BestowForm) -> Option<TargetDomain> {
        (form == BestowForm::Aura).then_some(TargetDomain::BattlefieldCreature)
    }
    pub fn characteristics(&self, form: BestowForm) -> Characteristics {
        let mut c = self.printed.clone();
        if form == BestowForm::Aura {
            c.kind = CardKind::Enchantment;
            c.additional_types.clear();
            c.subtypes = vec!["Aura".into()];
            c.power = None;
            c.toughness = None;
        }
        c
    }
    /// A missing announcement is invalid. An announced target which later
    /// becomes illegal makes the spell resolve in its original form.
    pub fn resolving_form(
        &self,
        form: BestowForm,
        legal_targets: &[bool],
    ) -> Result<BestowForm, String> {
        match (form, legal_targets) {
            (BestowForm::Creature, []) | (BestowForm::Aura, [false]) => Ok(BestowForm::Creature),
            (BestowForm::Aura, [true]) => Ok(BestowForm::Aura),
            _ => Err("bestow invocation target count differs from its announced form".into()),
        }
    }
    pub fn detach(&self) -> Characteristics {
        self.characteristics(BestowForm::Creature)
    }
}

pub fn bestow_alternative(d: &Definition) -> Option<(&ManaCost, &SpellProgram)> {
    d.static_abilities.iter().find_map(|a| match &a.effect {
        StaticEffect::Bestow { cost, program } => Some((cost, program)),
        _ => None,
    })
}
