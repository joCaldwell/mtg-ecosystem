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
    /// CR 700.5: count matching mana symbols in permanents this player controls.
    Devotion {
        player: PlayerExpr,
        colors: Vec<Color>,
    },
    /// The life total of one player.
    LifeOf {
        player: PlayerExpr,
    },
    /// CR 202.3, 601.2i: the total mana value of the spells `caster` cast
    /// this turn other than the resolving spell.
    OtherCastsManaValue {
        caster: PlayerExpr,
    },
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
    /// CR 107.3: the value announced for X.
    pub fn announced_x() -> Self {
        Self::Var {
            of: Ref::AnnouncedX,
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
