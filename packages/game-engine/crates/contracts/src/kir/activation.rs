//! CR 602.1b, 602.5 (2026-09-25): "Activate only if [condition]" is an
//! activation instruction. A player can't begin to activate an ability whose
//! condition is false; the condition is read from the current game state as
//! the activation begins and never again for that activation. Mana abilities
//! follow the same instruction (CR 605.3a).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

/// The audited current-state conditions an activation instruction can
/// read. The IR stores a [`Cond`]; [`ActivationCondition::of`] admits it only
/// in exactly one of these forms. Each leaf is one count compared with a
/// fixed number, so the core records the counts it saw as evidence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ActivationCondition {
    /// "if you control a [filter]" / "N or more [filter]" (CR 109.5,
    /// 608.2h): phased-in permanents the activator controls.
    Controlled {
        filter: Pred,
        cmp: Cmp,
        value: Quantity,
    },
    /// "if there are N or more [type] cards in your graveyard" (CR 404).
    GraveyardCards {
        filter: Pred,
        cmp: Cmp,
        value: Quantity,
    },
    /// "if you have no cards in hand" / "exactly N cards in hand" (CR 402).
    HandCards { cmp: Cmp, value: Quantity },
    /// "if this creature is attacking" / "if this creature's power is N or
    /// greater" (CR 506.4, 208.1): the source's own current facts. Its
    /// count is one when the source satisfies `filter`, otherwise zero.
    Source { filter: Pred },
    /// "if you created a token this turn" / "if a creature died this turn"
    /// (CR 500, 601.2i, 111.2): one of the activator's turn facts.
    TurnHistory {
        fact: TurnFact,
        cmp: Cmp,
        value: Quantity,
    },
    /// "if an opponent controls more lands than you" (CR 102.2-102.3). Its
    /// count is one when some opponent does, otherwise zero.
    OpponentControlsMore { kind: CardKind },
    /// "if an opponent controls N or more [filter]" (CR 102.2-102.3, 109.5):
    /// the most phased-in matching permanents one opponent controls.
    OpponentControls { filter: Pred, value: Quantity },
    /// "if you control N or more [filter] with the same name" (CR 201.2):
    /// the largest group of matching permanents you control sharing a name.
    SharedName { filter: Pred, value: Quantity },
    /// "if there are N or more card types among cards in your graveyard"
    /// (CR 205.2a, 404).
    GraveyardCardTypes { value: Quantity },
    /// "if you have at least N life more than your starting life total"
    /// (CR 103.4, 119.1): how far the life total exceeds the starting one.
    LifeAboveStarting { value: Quantity },
    /// "if creatures you control have total power N or greater" (CR 208.1,
    /// 107.1b: a negative power counts as itself).
    TotalPower { filter: Pred, value: Quantity },
    /// "if this land entered this turn" (CR 400.7): the source's current
    /// incarnation entered the battlefield this turn. One or zero.
    SourceEnteredThisTurn,
    /// Every part holds.
    All(Vec<ActivationCondition>),
    /// "or": at least one part holds.
    Any(Vec<ActivationCondition>),
}

/// A measurement read only as an activation begins (CR 602.5); its
/// filters name no controller and contain only
/// [`ActivationCondition::of`]'s admitted atoms.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "measure", rename_all = "snake_case", deny_unknown_fields)]
pub enum Measure {
    /// CR 102.2-102.3, 109.5: the most phased-in permanents satisfying
    /// `filter` that any one opponent of the activator controls.
    OpponentControls { filter: Pred },
    /// CR 201.2: the most phased-in permanents the activator controls
    /// satisfying `filter` that share one name.
    SharedName { filter: Pred },
    /// CR 103.4: how much the activator's life total exceeds their starting
    /// life total; zero when it does not.
    LifeAboveStarting {},
    /// CR 208.1, 107.1b: the total power of the phased-in permanents the
    /// activator controls satisfying `filter`; zero when negative.
    TotalPower { filter: Pred },
    /// CR 400.7: one when the source entered the battlefield this turn.
    SourceEnteredThisTurn {},
}

fn you() -> PlayerExpr {
    PlayerExpr::you(Context::Activated)
}

fn compare(set: SetExpr, cmp: Cmp, value: &Quantity) -> Cond {
    Cond::Compare {
        left: Amount::Count { set },
        cmp,
        right: Amount::fixed(value.clone()),
    }
}

fn holds(actual: u32, cmp: Cmp, value: &Quantity) -> bool {
    let actual = Quantity::from(i64::from(actual));
    match cmp {
        Cmp::AtLeast => actual >= *value,
        Cmp::AtMost => actual <= *value,
        Cmp::Exactly => actual == *value,
    }
}

/// The characteristic and status atoms the core reads with fully derived
/// facts (CR 613). Identity, ownership and combat atoms are not admitted.
fn permanent_atom(atom: &Atom) -> bool {
    match atom {
        Atom::Type { .. }
        | Atom::NotType { .. }
        | Atom::AnyType { .. }
        | Atom::Subtype { .. }
        | Atom::NotSubtype { .. }
        | Atom::Legendary
        | Atom::NotLegendary
        | Atom::Basic
        | Atom::NotBasic
        | Atom::Color { .. }
        | Atom::NotColor { .. }
        | Atom::Token
        | Atom::NotToken
        | Atom::Tapped
        | Atom::Untapped
        | Atom::Keyword {
            keyword: KeywordAbility::Flying,
        } => true,
        Atom::Power { value, .. } | Atom::Toughness { value, .. } => {
            value.as_fixed().is_some_and(|v| !v.is_negative())
        }
        Atom::AnyOf { alternatives } => {
            alternatives.len() >= 2
                && alternatives
                    .iter()
                    .all(|p| !p.0.is_empty() && p.0.iter().all(permanent_atom))
        }
        _ => false,
    }
}

/// CR 506.4, 110.5, 208.1: combat status, tapped status and fixed power
/// or toughness bounds of the source itself.
fn source_atom(atom: &Atom) -> bool {
    match atom {
        Atom::Attacking | Atom::Blocking | Atom::Tapped | Atom::Untapped => true,
        Atom::Power { value, .. } | Atom::Toughness { value, .. } => {
            value.as_fixed().is_some_and(|v| !v.is_negative())
        }
        Atom::AnyOf { alternatives } => {
            alternatives.len() >= 2
                && alternatives
                    .iter()
                    .all(|p| !p.0.is_empty() && p.0.iter().all(source_atom))
        }
        _ => false,
    }
}

/// CR 404.2: a graveyard card's printed types and subtypes.
fn card_atom(atom: &Atom) -> bool {
    match atom {
        Atom::Type { .. } | Atom::NotType { .. } | Atom::AnyType { .. } | Atom::Subtype { .. } => {
            true
        }
        Atom::AnyOf { alternatives } => {
            alternatives.len() >= 2
                && alternatives
                    .iter()
                    .all(|p| !p.0.is_empty() && p.0.iter().all(card_atom))
        }
        _ => false,
    }
}

impl ActivationCondition {
    /// The kernel condition this form names.
    pub fn condition(&self) -> Cond {
        match self {
            Self::Controlled { filter, cmp, value } => {
                compare(SetExpr::battlefield(filter.clone()), *cmp, value)
            }
            Self::GraveyardCards { filter, cmp, value } => compare(
                SetExpr::Matching {
                    zone: ZoneExpr::Graveyard { of: you() },
                    filter: filter.clone(),
                },
                *cmp,
                value,
            ),
            Self::HandCards { cmp, value } => compare(
                SetExpr::Matching {
                    zone: ZoneExpr::Hand { of: you() },
                    filter: Pred::default(),
                },
                *cmp,
                value,
            ),
            Self::Source { filter } => Cond::Satisfies {
                object: Ref::Source,
                filter: filter.clone(),
            },
            Self::TurnHistory { fact, cmp, value } => Cond::TurnHistory {
                fact: *fact,
                cmp: *cmp,
                value: value.clone(),
            },
            Self::OpponentControlsMore { kind } => Cond::OpponentControlsMore { kind: *kind },
            Self::OpponentControls { filter, value } => Cond::Measured {
                measure: Measure::OpponentControls {
                    filter: filter.clone(),
                },
                cmp: Cmp::AtLeast,
                value: value.clone(),
            },
            Self::SharedName { filter, value } => Cond::Measured {
                measure: Measure::SharedName {
                    filter: filter.clone(),
                },
                cmp: Cmp::AtLeast,
                value: value.clone(),
            },
            Self::GraveyardCardTypes { value } => Cond::Compare {
                left: Amount::Distinct {
                    set: SetExpr::Matching {
                        zone: ZoneExpr::Graveyard { of: you() },
                        filter: Pred::default(),
                    },
                    characteristic: DistinctCharacteristic::CardType,
                },
                cmp: Cmp::AtLeast,
                right: Amount::fixed(value.clone()),
            },
            Self::TotalPower { filter, value } => Cond::Measured {
                measure: Measure::TotalPower {
                    filter: filter.clone(),
                },
                cmp: Cmp::AtLeast,
                value: value.clone(),
            },
            Self::SourceEnteredThisTurn => Cond::Measured {
                measure: Measure::SourceEnteredThisTurn {},
                cmp: Cmp::AtLeast,
                value: 1.into(),
            },
            Self::LifeAboveStarting { value } => Cond::Measured {
                measure: Measure::LifeAboveStarting {},
                cmp: Cmp::AtLeast,
                value: value.clone(),
            },
            Self::All(parts) => Cond::All {
                conditions: parts.iter().map(Self::condition).collect(),
            },
            Self::Any(parts) => Cond::Not {
                condition: Box::new(Cond::All {
                    conditions: parts
                        .iter()
                        .map(|p| Cond::Not {
                            condition: Box::new(p.condition()),
                        })
                        .collect(),
                }),
            },
        }
    }

    /// The audited evaluation `condition` names, or the reason it has none.
    pub fn of(condition: &Cond) -> Result<Self, KirError> {
        let unaudited = || KirError::Unaudited("activation condition".into());
        let candidate = match condition {
            Cond::Compare {
                left:
                    Amount::Count {
                        set: SetExpr::Matching { zone, filter },
                    },
                cmp,
                right: Amount::Fixed { value },
            } if !value.is_negative() => match zone {
                ZoneExpr::Battlefield => {
                    let controller = Atom::ControlledBy { player: you() };
                    let rest: Vec<_> = filter.0.iter().filter(|a| **a != controller).collect();
                    if rest.len() + 1 != filter.0.len() || !rest.iter().all(|a| permanent_atom(a)) {
                        return Err(unaudited());
                    }
                    Self::Controlled {
                        filter: filter.clone(),
                        cmp: *cmp,
                        value: value.clone(),
                    }
                }
                ZoneExpr::Graveyard { .. } if filter.0.iter().all(card_atom) => {
                    Self::GraveyardCards {
                        filter: filter.clone(),
                        cmp: *cmp,
                        value: value.clone(),
                    }
                }
                ZoneExpr::Hand { .. } => Self::HandCards {
                    cmp: *cmp,
                    value: value.clone(),
                },
                _ => return Err(unaudited()),
            },
            Cond::Compare {
                left:
                    Amount::Distinct {
                        characteristic: DistinctCharacteristic::CardType,
                        ..
                    },
                cmp: Cmp::AtLeast,
                right: Amount::Fixed { value },
            } if !value.is_negative() && *value != Quantity::zero() => Self::GraveyardCardTypes {
                value: value.clone(),
            },
            // CR 500: the activator's own facts this turn.
            Cond::TurnHistory { fact, cmp, value } if !value.is_negative() => Self::TurnHistory {
                fact: *fact,
                cmp: *cmp,
                value: value.clone(),
            },
            Cond::OpponentControlsMore {
                kind: kind @ (CardKind::Land | CardKind::Creature),
            } => Self::OpponentControlsMore { kind: *kind },
            Cond::Measured {
                measure,
                cmp: Cmp::AtLeast,
                value,
            } if !value.is_negative() && *value != Quantity::zero() => match measure {
                Measure::OpponentControls { filter }
                    if !filter.0.is_empty() && filter.0.iter().all(permanent_atom) =>
                {
                    Self::OpponentControls {
                        filter: filter.clone(),
                        value: value.clone(),
                    }
                }
                Measure::SharedName { filter } | Measure::TotalPower { filter } => {
                    let controller = Atom::ControlledBy { player: you() };
                    let rest: Vec<_> = filter.0.iter().filter(|a| **a != controller).collect();
                    if rest.len() + 1 != filter.0.len()
                        || rest.is_empty()
                        || !rest.iter().all(|a| permanent_atom(a))
                    {
                        return Err(unaudited());
                    }
                    if matches!(measure, Measure::TotalPower { .. }) {
                        Self::TotalPower {
                            filter: filter.clone(),
                            value: value.clone(),
                        }
                    } else {
                        Self::SharedName {
                            filter: filter.clone(),
                            value: value.clone(),
                        }
                    }
                }
                Measure::SourceEnteredThisTurn {} if *value == Quantity::from(1) => {
                    Self::SourceEnteredThisTurn
                }
                Measure::LifeAboveStarting {} => Self::LifeAboveStarting {
                    value: value.clone(),
                },
                _ => return Err(unaudited()),
            },
            Cond::Satisfies {
                object: Ref::Source,
                filter,
            } if !filter.0.is_empty() && filter.0.iter().all(source_atom) => Self::Source {
                filter: filter.clone(),
            },
            Cond::All { conditions } if conditions.len() >= 2 => {
                Self::All(conditions.iter().map(Self::of).collect::<Result<_, _>>()?)
            }
            Cond::Not { condition } => match &**condition {
                Cond::All { conditions } if conditions.len() >= 2 => Self::Any(
                    conditions
                        .iter()
                        .map(|c| match c {
                            Cond::Not { condition } => Self::of(condition),
                            _ => Err(unaudited()),
                        })
                        .collect::<Result<_, _>>()?,
                ),
                _ => return Err(unaudited()),
            },
            _ => return Err(unaudited()),
        };
        if candidate.condition() == *condition && candidate.leaves().iter().all(|l| l.exact()) {
            Ok(candidate)
        } else {
            Err(unaudited())
        }
    }

    /// Whether this leaf's comparison has the same outcome on its evidence
    /// count as on the measured amount. Evidence is a `u32`; life gained,
    /// total power and life above the start are clamped into that range, so
    /// only "at least" a threshold within it compares exactly. Every other
    /// measure is an exact count.
    fn exact(&self) -> bool {
        let value = match self {
            Self::TurnHistory {
                fact: TurnFact::LifeGained,
                cmp,
                ..
            } if *cmp != Cmp::AtLeast => return false,
            Self::Controlled { value, .. }
            | Self::GraveyardCards { value, .. }
            | Self::HandCards { value, .. }
            | Self::TurnHistory { value, .. }
            | Self::OpponentControls { value, .. }
            | Self::SharedName { value, .. }
            | Self::GraveyardCardTypes { value }
            | Self::LifeAboveStarting { value }
            | Self::TotalPower { value, .. } => value,
            Self::OpponentControlsMore { .. }
            | Self::Source { .. }
            | Self::SourceEnteredThisTurn
            | Self::All(_)
            | Self::Any(_) => return true,
        };
        value.to_u32().is_some()
    }

    /// The counted leaves in printed order; the core measures each one.
    pub fn leaves(&self) -> Vec<&Self> {
        match self {
            Self::All(parts) | Self::Any(parts) => parts.iter().flat_map(Self::leaves).collect(),
            leaf => vec![leaf],
        }
    }

    /// Whether the counts measured for [`Self::leaves`] satisfy this form.
    /// A wrong number of counts never holds.
    pub fn passes(&self, counts: &[u32]) -> bool {
        let mut rest = counts;
        self.consume(&mut rest).unwrap_or(false) && rest.is_empty()
    }

    fn consume(&self, counts: &mut &[u32]) -> Option<bool> {
        Some(match self {
            Self::Controlled { cmp, value, .. }
            | Self::GraveyardCards { cmp, value, .. }
            | Self::HandCards { cmp, value }
            | Self::TurnHistory { cmp, value, .. } => {
                let (first, rest) = counts.split_first()?;
                *counts = rest;
                holds(*first, *cmp, value)
            }
            Self::OpponentControls { value, .. }
            | Self::SharedName { value, .. }
            | Self::TotalPower { value, .. }
            | Self::GraveyardCardTypes { value }
            | Self::LifeAboveStarting { value } => {
                let (first, rest) = counts.split_first()?;
                *counts = rest;
                holds(*first, Cmp::AtLeast, value)
            }
            Self::OpponentControlsMore { .. }
            | Self::Source { .. }
            | Self::SourceEnteredThisTurn => {
                let (first, rest) = counts.split_first()?;
                *counts = rest;
                match first {
                    0 => false,
                    1 => true,
                    _ => return None,
                }
            }
            Self::All(parts) => {
                let mut all = true;
                for p in parts {
                    all &= p.consume(counts)?;
                }
                all
            }
            Self::Any(parts) => {
                let mut any = false;
                for p in parts {
                    any |= p.consume(counts)?;
                }
                any
            }
        })
    }
}
