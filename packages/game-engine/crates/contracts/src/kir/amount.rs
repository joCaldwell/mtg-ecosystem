//! Amounts: fixed, referenced and computed quantities (CR 107.1-107.3).
use super::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "amount", rename_all = "snake_case", deny_unknown_fields)]
pub enum Amount {
    Fixed {
        value: Quantity,
    },
    /// A number a reference denotes: announced X, a step result, a bound
    /// variable.
    Var {
        of: Ref,
    },
    /// CR 608.2h: the number of objects in a set as the verb is performed.
    Count {
        set: SetExpr,
    },
    /// A characteristic of one object, using last known information if it
    /// has left (CR 608.2h).
    StatOf {
        object: Ref,
        stat: Stat,
    },
    /// CR 608.2h: the greatest `stat` among the objects of a set as the verb
    /// is performed; zero when the set is empty.
    Greatest {
        set: SetExpr,
        stat: Stat,
    },
    /// CR 608.2h: "N for each [filter]": `factor` times a resolution count.
    Times {
        factor: Quantity,
        of: Box<Amount>,
    },
    /// CR 700.5: count matching mana symbols in permanents this player controls.
    Devotion {
        player: PlayerExpr,
        colors: Vec<Color>,
    },
    /// The life total of one player.
    LifeOf {
        player: PlayerExpr,
    },
    /// CR 122.1, 608.2h: how many counters of `counter` are on one object
    /// as the verb reads it, using last known information if it has left.
    CountersOn {
        object: Ref,
        counter: CounterKind,
    },
    /// CR 509.1g, 702.23a: the number of creatures blocking `object` as the
    /// verb is performed, less `beyond`, never below zero.
    BlockersOf {
        object: Ref,
        beyond: u32,
    },
    /// CR 702.121a, 508.1: the number of opponents of the invocation's
    /// controller that player declared a creature attacking this combat.
    /// A creature put onto the battlefield attacking never attacked
    /// (CR 508.4).
    OpponentsAttacked {},
    /// CR 202.3, 601.2i: the total mana value of the spells `caster` cast
    /// this turn other than the resolving spell.
    OtherCastsManaValue {
        caster: PlayerExpr,
    },
    /// CR 107.1, 208.2a: the sum of two or more numbers.
    Sum {
        terms: Vec<Amount>,
    },
    /// CR 107.1b: `minuend` minus `subtrahend`. The difference stays signed
    /// until the verb that reads it applies its own quantity rule.
    Difference {
        minuend: Box<Amount>,
        subtrahend: Box<Amount>,
    },
    /// CR 205.2a, 205.3i, 105.1: how many different card types, basic land
    /// types or colors the objects of a set have among them. Read only by a
    /// characteristic-defining ability (`kir::statics`).
    Distinct {
        set: SetExpr,
        characteristic: DistinctCharacteristic,
    },
}

/// The characteristic an [`Amount::Distinct`] counts the values of.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DistinctCharacteristic {
    CardType,
    BasicLandType,
    Color,
}

impl Amount {
    pub fn fixed(value: impl Into<Quantity>) -> Self {
        Self::Fixed {
            value: value.into(),
        }
    }
    pub fn as_fixed(&self) -> Option<&Quantity> {
        match self {
            Self::Fixed { value } => Some(value),
            _ => None,
        }
    }
    /// The fixed draw counts covered by the original single-draw review.
    pub(crate) fn is_reviewed_fixed_draw(&self) -> bool {
        self.as_fixed()
            .is_some_and(|value| *value == Quantity::from(1) || *value == Quantity::from(2))
    }
    /// CR 107.3: the value announced for X.
    pub fn announced_x() -> Self {
        Self::Var {
            of: Ref::AnnouncedX,
        }
    }
    /// CR 608.2h: a number the game state supplies when the verb reads it.
    pub fn is_resolution_count(&self) -> bool {
        match self {
            Self::Count { .. }
            | Self::Greatest { .. }
            | Self::Devotion { .. }
            | Self::CountersOn { .. }
            | Self::BlockersOf { .. }
            | Self::OpponentsAttacked {} => true,
            Self::Times { of, .. } => of.is_resolution_count(),
            _ => false,
        }
    }
    /// The count of a hand this number reads, which a draw changes.
    pub fn counts_hand(&self) -> bool {
        match self {
            Self::Count {
                set: SetExpr::Matching { zone, .. },
            } => matches!(zone, ZoneExpr::Hand { .. }),
            Self::Times { of, .. } => of.counts_hand(),
            _ => false,
        }
    }
    pub fn is_announced_x(&self) -> bool {
        matches!(
            self,
            Self::Var {
                of: Ref::AnnouncedX
            }
        )
    }
}
