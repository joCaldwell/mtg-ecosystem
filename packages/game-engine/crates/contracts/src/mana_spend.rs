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
    ArtifactSpellOrArtifactAbility,
    ArtifactAbility,
    ColorlessSpell,
    ColorlessEldraziSpellOrAbility,
    SpellType { kind: CardKind },
    LegendarySpellUncounterable,
    InstantOrSorceryUncounterable,
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
    types: u8,
    eldrazi: bool,
    legendary: bool,
}

impl ManaSpellTraits {
    fn has_type(self, kind: CardKind) -> bool {
        self.types & (1 << kind as u8) != 0
    }

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
            types: [
                CardKind::Land,
                CardKind::Creature,
                CardKind::Instant,
                CardKind::Sorcery,
                CardKind::Enchantment,
                CardKind::Artifact,
            ]
            .into_iter()
            .filter(|kind| c.has_type(*kind))
            .fold(0, |bits, kind| bits | (1 << kind as u8)),
            eldrazi: c.has_subtype("Eldrazi"),
            legendary: c.legendary,
        }
    }
}

impl ManaSpendRestriction {
    pub fn is_canonical(self) -> bool {
        match self {
            Self::SpellSubtypes { any_of } => any_of == SPELL_SUBTYPES,
            Self::SpellType { kind } => matches!(
                kind,
                CardKind::Creature
                    | CardKind::Artifact
                    | CardKind::Enchantment
                    | CardKind::Instant
                    | CardKind::Sorcery
            ),
            _ => true,
        }
    }

    // CR 109.2b and 106.6: this tests the spell on the stack, using subtype
    // membership. An Equipment subtype differs from the artifact card type.
    pub fn permits_spell(self, spell: ManaSpellTraits) -> bool {
        match self {
            Self::ActivatedAbility | Self::ArtifactAbility => false,
            Self::ArtifactSpellOrArtifactAbility => spell.has_type(CardKind::Artifact),
            Self::SpellType { kind } => spell.has_type(kind),
            Self::ColorlessSpell => spell.colorless,
            Self::ColorlessEldraziSpellOrAbility => spell.colorless && spell.eldrazi,
            Self::LegendarySpellUncounterable => spell.legendary,
            Self::InstantOrSorceryUncounterable => true,
            Self::ColorlessSpellOrActivatedAbility => spell.colorless,
            Self::CreatureSpellOrBattlefieldCreatureAbility => spell.creature,
            Self::SpellSubtypes { any_of } => any_of
                .into_iter()
                .any(|subtype| spell.subtypes & (1 << subtype.index()) != 0),
        }
    }

    pub fn prevents_countering(self, spell: ManaSpellTraits) -> bool {
        match self {
            Self::LegendarySpellUncounterable => spell.legendary,
            Self::InstantOrSorceryUncounterable => {
                spell.has_type(CardKind::Instant) || spell.has_type(CardKind::Sorcery)
            }
            _ => false,
        }
    }

    pub fn permits_activation_of(self, source: ManaSpellTraits, battlefield: bool) -> bool {
        match self {
            Self::ArtifactAbility | Self::ArtifactSpellOrArtifactAbility => {
                battlefield && source.has_type(CardKind::Artifact)
            }
            Self::ColorlessEldraziSpellOrAbility => {
                battlefield && source.colorless && source.eldrazi
            }
            _ => self.permits_activation(battlefield && source.creature),
        }
    }

    pub fn permits_resolution_payment(self) -> bool {
        self == Self::InstantOrSorceryUncounterable
    }

    pub fn permits_activation(self, battlefield_creature: bool) -> bool {
        matches!(
            self,
            Self::ActivatedAbility
                | Self::ColorlessSpellOrActivatedAbility
                | Self::InstantOrSorceryUncounterable
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spell_type_restrictions_accept_only_supported_spell_types() {
        for kind in CardKind::CARD_TYPES {
            assert_eq!(
                ManaSpendRestriction::SpellType { kind }.is_canonical(),
                !matches!(kind, CardKind::Land | CardKind::Planeswalker),
                "{kind:?}"
            );
        }
    }
}
