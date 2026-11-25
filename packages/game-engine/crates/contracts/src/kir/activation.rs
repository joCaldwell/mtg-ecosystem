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
    /// Every part holds.
    All(Vec<ActivationCondition>),
    /// "or": at least one part holds.
    Any(Vec<ActivationCondition>),
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
                left: Amount::Count {
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
            Cond::All { conditions } if conditions.len() >= 2 => Self::All(
                conditions
                    .iter()
                    .map(Self::of)
                    .collect::<Result<_, _>>()?,
            ),
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
        if candidate.condition() == *condition {
            Ok(candidate)
        } else {
            Err(unaudited())
        }
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
            | Self::HandCards { cmp, value } => {
                let (first, rest) = counts.split_first()?;
                *counts = rest;
                holds(*first, *cmp, value)
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
