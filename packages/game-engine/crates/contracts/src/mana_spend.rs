//! Restricted mana remains restricted until spent or lost (CR 106.6).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use std::collections::BTreeMap;

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
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
            if c.has_subtype(subtype.printed()) {
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

pub fn spend(
    cost: &ManaCost,
    available: &BTreeMap<Color, Quantity>,
) -> Option<BTreeMap<Color, Quantity>> {
    let used = payment(cost, available)?;
    Some(
        PAYMENT_ORDER
            .into_iter()
            .zip(used)
            .filter(|(_, n)| *n > Quantity::zero())
            .collect(),
    )
}

const PAYMENT_ORDER: [Color; 6] = [Color::B, Color::G, Color::W, Color::U, Color::R, Color::C];

pub fn payment(cost: &ManaCost, available: &BTreeMap<Color, Quantity>) -> Option<[Quantity; 6]> {
    let mut used: [Quantity; 6] = Default::default();
    let one = Quantity::from(1);
    for c in &cost.colored {
        let index = match c {
            Color::B => 0,
            Color::G => 1,
            Color::W => 2,
            Color::U => 3,
            Color::R => 4,
            Color::C => 5,
        };
        let n = &mut used[index];
        n.add(&one);
        if &*n > available.get(c)? {
            return None;
        }
    }
    let mut rest = cost.generic.clone();
    for (c, already) in PAYMENT_ORDER.into_iter().zip(&mut used) {
        if rest == Quantity::zero() {
            break;
        }
        let Some(mut n) = available.get(&c).cloned() else {
            continue;
        };
        n.subtract(already);
        let n = n.min(rest.clone());
        if n > Quantity::zero() {
            rest.subtract(&n);
            already.add(&n);
        }
    }
    (rest == Quantity::zero()).then_some(used)
}
