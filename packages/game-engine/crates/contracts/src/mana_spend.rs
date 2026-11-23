//! Restricted mana remains restricted until spent or lost (CR 106.6).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ManaSpendRestriction {
    ActivatedAbility,
    ColorlessSpellOrActivatedAbility,
    CreatureSpellOrBattlefieldCreatureAbility,
    SpellSubtypes { any_of: [ManaSpellSubtype; 3] },
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ManaSpellSubtype {
    Dwarf,
    Equipment,
    Saga,
}

impl ManaSpellSubtype {
    fn printed(self) -> &'static str {
        match self {
            Self::Dwarf => "Dwarf",
            Self::Equipment => "Equipment",
            Self::Saga => "Saga",
        }
    }

    fn index(self) -> u8 {
        match self {
            Self::Dwarf => 0,
            Self::Equipment => 1,
            Self::Saga => 2,
        }
    }
}

const SPELL_SUBTYPES: [ManaSpellSubtype; 3] = [
    ManaSpellSubtype::Dwarf,
    ManaSpellSubtype::Equipment,
    ManaSpellSubtype::Saga,
];

/// Only characteristics consulted by the supported spending predicates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ManaSpellTraits {
    colorless: bool,
    creature: bool,
    subtypes: u8,
}

impl ManaSpellTraits {
    pub fn from_characteristics(c: &Characteristics) -> Self {
        let mut subtypes = 0;
        for subtype in SPELL_SUBTYPES {
            if c.subtypes.iter().any(|name| name == subtype.printed()) {
                subtypes |= 1 << subtype.index();
            }
        }
        Self {
            colorless: c.colors.is_empty(),
            creature: c.has_type(CardKind::Creature),
            subtypes,
        }
    }
}

impl ManaSpendRestriction {
    pub fn is_canonical(self) -> bool {
        match self {
            Self::SpellSubtypes { any_of } => any_of == SPELL_SUBTYPES,
            _ => true,
        }
    }

    // CR 109.2b and 106.6: this tests the spell on the stack, using subtype
    // membership. An Equipment subtype differs from the artifact card type.
    pub fn permits_spell(self, spell: ManaSpellTraits) -> bool {
        match self {
            Self::ActivatedAbility => false,
            Self::ColorlessSpellOrActivatedAbility => spell.colorless,
            Self::CreatureSpellOrBattlefieldCreatureAbility => spell.creature,
            Self::SpellSubtypes { any_of } => any_of
                .into_iter()
                .any(|subtype| spell.subtypes & (1 << subtype.index()) != 0),
        }
    }

    pub fn permits_activation(self, battlefield_creature: bool) -> bool {
        matches!(
            self,
            Self::ActivatedAbility | Self::ColorlessSpellOrActivatedAbility
        ) || battlefield_creature && self == Self::CreatureSpellOrBattlefieldCreatureAbility
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RestrictedMana {
    pub restriction: ManaSpendRestriction,
    pub color: Color,
    pub amount: Quantity,
}

impl RestrictedMana {
    pub fn key(&self) -> (ManaSpendRestriction, Color) {
        (self.restriction, self.color)
    }
}

use serde_json::{Value, json};

const PREFIX: &str = "{T}: Add ";

const INVALID: &str = "requires complete fixed tap mana and exact printed spending predicate";

pub(super) fn present(d: &Definition) -> bool {
    d.mana_abilities
        .iter()
        .any(|a| matches!(a.production, ManaProduction::RestrictedFixed { .. }))
}

fn permanent_spell() -> SpellProgram {
    SpellProgram {
        functioning_zone: FunctioningZone::Stack,
        timing: SpellTiming::MainPhase,
        resolution: SpellResolution::Permanent,
        targets: vec![],
        instructions: vec![],
        modes: vec![],
        optional_additional_costs: vec![],
        additional_sacrifice: None,
    }
}
