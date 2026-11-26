//! Predicates, zones and object sets (CR 608.2h).
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Cmp {
    AtLeast,
    AtMost,
    Exactly,
}

/// One conjunct of an object predicate. Card types, supertypes and subtypes
/// are the object's current ones (CR 205, 613.1d); colors are current colors
/// (CR 105, 613.1e); power, toughness and keywords are fully derived
/// (CR 613.4, 613.1f).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "atom", rename_all = "snake_case", deny_unknown_fields)]
pub enum Atom {
    Type {
        kind: CardKind,
    },
    NotType {
        kind: CardKind,
    },
    /// Has at least one of the listed card types.
    AnyType {
        kinds: Vec<CardKind>,
    },
    Subtype {
        subtype: String,
    },
    NotSubtype {
        subtype: String,
    },
    #[serde(deserialize_with = "no_operands")]
    Legendary,
    #[serde(deserialize_with = "no_operands")]
    NotLegendary,
    #[serde(deserialize_with = "no_operands")]
    Basic,
    #[serde(deserialize_with = "no_operands")]
    NotBasic,
    Color {
        color: Color,
    },
    NotColor {
        color: Color,
    },
    /// CR 109.5: controlled by one of the denoted players.
    ControlledBy {
        player: PlayerExpr,
    },
    OwnedBy {
        player: PlayerExpr,
    },
    #[serde(deserialize_with = "no_operands")]
    Tapped,
    #[serde(deserialize_with = "no_operands")]
    Untapped,
    /// CR 506.4: an attacking creature.
    #[serde(deserialize_with = "no_operands")]
    Attacking,
    #[serde(deserialize_with = "no_operands")]
    Blocking,
    #[serde(deserialize_with = "no_operands")]
    Token,
    #[serde(deserialize_with = "no_operands")]
    NotToken,
    Keyword {
        keyword: KeywordAbility,
    },
    NotKeyword {
        keyword: KeywordAbility,
    },
    Power {
        cmp: Cmp,
        value: Amount,
    },
    Toughness {
        cmp: Cmp,
        value: Amount,
    },
    ManaValue {
        cmp: Cmp,
        value: Amount,
    },
    /// CR 201.5: the referenced object itself. Only an event pattern
    /// compares its object with a reference this way.
    Is {
        object: Ref,
    },
    /// "Other", "another": not the referenced object.
    IsNot {
        object: Ref,
    },
    /// Satisfies at least one alternative.
    AnyOf {
        alternatives: Vec<Pred>,
    },
}

/// A conjunction of atoms. The empty predicate admits every object of the
/// zone it is evaluated in.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct Pred(pub Vec<Atom>);

impl Pred {
    /// True when the predicate has no atoms and admits every object.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "zone", rename_all = "snake_case", deny_unknown_fields)]
pub enum ZoneExpr {
    /// CR 110.1, 702.26b: phased-in permanents.
    #[serde(deserialize_with = "no_operands")]
    Battlefield,
    /// Every graveyard (CR 404).
    #[serde(deserialize_with = "no_operands")]
    Graveyards,
    Graveyard {
        of: PlayerExpr,
    },
    Hand {
        of: PlayerExpr,
    },
    Library {
        of: PlayerExpr,
    },
    #[serde(deserialize_with = "no_operands")]
    Exile,
    #[serde(deserialize_with = "no_operands")]
    Stack,
}

impl ZoneExpr {
    /// CR 400.2: hands and libraries are hidden zones.
    pub fn hidden(&self) -> bool {
        matches!(self, Self::Hand { .. } | Self::Library { .. })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "set", rename_all = "snake_case", deny_unknown_fields)]
pub enum SetExpr {
    /// The single object a reference denotes.
    One { object: Ref },
    /// CR 608.2h: every object of `zone` that satisfies `filter` at the
    /// moment the verb is performed.
    Matching { zone: ZoneExpr, filter: Pred },
    /// The cards of a linked pool.
    Pool { pool: Ref },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Stat {
    Power,
    Toughness,
    ManaValue,
}

impl Pred {
    /// "Lands you control" (CR 205.2a, 109.5).
    pub fn lands_you_control(context: Context) -> Self {
        Self(vec![
            Atom::Type {
                kind: CardKind::Land,
            },
            Atom::ControlledBy {
                player: PlayerExpr::you(context),
            },
        ])
    }
    /// "Creatures" (CR 205.2a).
    pub fn creatures() -> Self {
        Self(vec![Atom::Type {
            kind: CardKind::Creature,
        }])
    }
    /// "Nonland permanent" (CR 110.1, 205.2a).
    pub fn nonland_permanents() -> Self {
        Self(vec![Atom::NotType {
            kind: CardKind::Land,
        }])
    }
    /// True when some conjunct names the invocation's controller.
    pub fn names_you(&self) -> bool {
        self.0.iter().any(|atom| match atom {
            Atom::ControlledBy { player } | Atom::OwnedBy { player } => player.is_you(),
            Atom::AnyOf { alternatives } => alternatives.iter().any(Self::names_you),
            _ => false,
        })
    }
}

impl SetExpr {
    /// CR 701.26b: the lands "you" in `context` control, on the battlefield.
    pub fn lands_you_control(context: Context) -> Self {
        Self::battlefield(Pred::lands_you_control(context))
    }
    /// Every card in one player's graveyard (CR 404.1).
    pub fn graveyard_of(player: PlayerExpr) -> Self {
        Self::Matching {
            zone: ZoneExpr::Graveyard { of: player },
            filter: Pred::default(),
        }
    }
    /// Every card in every graveyard (CR 404).
    pub fn graveyards() -> Self {
        Self::Matching {
            zone: ZoneExpr::Graveyards,
            filter: Pred::default(),
        }
    }
    /// The filter of a board-wide or graveyard-wide set.
    pub fn filter(&self) -> Option<&Pred> {
        match self {
            Self::Matching { filter, .. } => Some(filter),
            _ => None,
        }
    }
    pub fn target(slot: u32) -> Self {
        Self::One {
            object: Ref::Target { slot },
        }
    }
    pub fn source() -> Self {
        Self::One {
            object: Ref::Source,
        }
    }
    pub fn battlefield(filter: Pred) -> Self {
        Self::Matching {
            zone: ZoneExpr::Battlefield,
            filter,
        }
    }
    pub fn target_slot(&self) -> Option<u32> {
        match self {
            Self::One {
                object: Ref::Target { slot },
            } => Some(*slot),
            _ => None,
        }
    }
}
