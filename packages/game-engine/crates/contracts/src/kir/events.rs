//! Trigger events: what an ordinary triggered ability looks for (CR 603.2),
//! and the intervening "if" it checks twice (CR 603.4).
//!
//! An event names its object or actor with the operand algebra the rest of
//! the kernel uses. An object operand is a [`Pred`] the event's object must
//! satisfy, relative to the ability's controller ("you", CR 109.5) and its
//! source: [`Atom::Is`] and [`Atom::IsNot`] compare the event's object with
//! [`Ref::Source`] (CR 201.5) or [`Ref::AttachedTo`] (CR 301.5, 303.4). A
//! player operand is "you", each opponent or each player: the player the
//! event names must be one of them.
//!
//! Pinned Comprehensive Rules 2026-09-25:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use crate::BeginningStep;

/// One event an ordinary triggered ability triggers on (CR 603.2).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "event", rename_all = "snake_case", deny_unknown_fields)]
pub enum EventPattern {
    /// CR 603.6a: a permanent enters the battlefield. Its object is matched
    /// on the battlefield the event results in.
    Enters { object: Pred },
    /// CR 700.4, 603.10a: a permanent is put into a graveyard from the
    /// battlefield. Its object is matched on the battlefield immediately
    /// before the event, including a source that dies at the same time.
    Dies { object: Pred },
    /// CR 701.26a: a permanent actually changes from untapped to tapped.
    BecomesTapped { object: Pred },
    /// CR 508.3a: a creature is declared as an attacker. It is matched on its
    /// facts immediately before the declaration.
    Attacks { attacker: Pred },
    /// CR 601.2i: a player casts a spell. The spell is matched on its frozen
    /// characteristics on the stack.
    Cast { caster: PlayerExpr, spell: Pred },
    /// CR 503.1a, 504.1, 505.1, 513.1: a step begins in the turn of a player
    /// in `active`. Each player also matches a turn whose active player has
    /// left the game (CR 800.4j).
    StepBegins {
        step: BeginningStep,
        active: PlayerExpr,
    },
    /// CR 120.3, 510.2: an object deals positive damage to a player in
    /// `recipient`. `combat` restricts it to combat damage. The source is
    /// matched on its battlefield appearance as the damage is dealt.
    DealsDamage {
        source: Pred,
        combat: bool,
        recipient: PlayerExpr,
    },
}

impl EventPattern {
    /// Stable reporting vocabulary, independent of the serde representation.
    pub fn reporting_key(&self) -> &'static str {
        match self {
            Self::Enters { .. } => "enters",
            Self::Dies { .. } => "dies",
            Self::BecomesTapped { .. } => "becomes_tapped",
            Self::Attacks { .. } => "attacks",
            Self::Cast { .. } => "cast",
            Self::StepBegins { .. } => "step_begins",
            Self::DealsDamage { .. } => "deals_damage",
        }
    }

    /// The predicate on the event's object, when the event has one.
    pub fn object(&self) -> Option<&Pred> {
        match self {
            Self::Enters { object } | Self::Dies { object } | Self::BecomesTapped { object } => {
                Some(object)
            }
            Self::Attacks { attacker } => Some(attacker),
            Self::Cast { spell, .. } => Some(spell),
            Self::DealsDamage { source, .. } => Some(source),
            Self::StepBegins { .. } => None,
        }
    }

    /// "When this permanent enters" (CR 603.6a, 201.5).
    pub fn self_enters() -> Self {
        Self::Enters {
            object: Pred::source(),
        }
    }
    /// "Whenever another creature enters" (CR 603.6a, 201.5).
    pub fn other_creature_enters() -> Self {
        Self::Enters {
            object: Pred(vec![
                Atom::Type {
                    kind: CardKind::Creature,
                },
                Atom::IsNot {
                    object: Ref::Source,
                },
            ]),
        }
    }
    /// "Whenever a land you control enters" (CR 603.6a, 109.5).
    pub fn controlled_land_enters() -> Self {
        Self::Enters {
            object: Pred::lands_you_control(Context::Triggered),
        }
    }
    /// "When this creature dies" (CR 700.4).
    pub fn self_dies() -> Self {
        Self::Dies {
            object: Pred(vec![
                Atom::Is {
                    object: Ref::Source,
                },
                Atom::Type {
                    kind: CardKind::Creature,
                },
            ]),
        }
    }
    /// "Whenever a creature an opponent controls dies" (CR 700.4, 102.3).
    pub fn opponent_creature_dies() -> Self {
        Self::Dies {
            object: Pred(vec![
                Atom::Type {
                    kind: CardKind::Creature,
                },
                Atom::ControlledBy {
                    player: PlayerExpr::EachOpponent,
                },
            ]),
        }
    }
    /// "Whenever this permanent becomes tapped" (CR 701.26a).
    pub fn self_becomes_tapped() -> Self {
        Self::BecomesTapped {
            object: Pred::source(),
        }
    }
}

impl Pred {
    /// The source itself (CR 201.5).
    pub fn source() -> Self {
        Self(vec![Atom::Is {
            object: Ref::Source,
        }])
    }
    /// The creature the source is attached to (CR 301.5, 303.4).
    pub fn attached_creature() -> Self {
        Self(vec![
            Atom::Is {
                object: Ref::AttachedTo,
            },
            Atom::Type {
                kind: CardKind::Creature,
            },
        ])
    }
    /// True when some conjunct, outside alternatives, names `object`
    /// with [`Atom::Is`].
    pub fn is_ref(&self, object: Ref) -> bool {
        self.0.contains(&Atom::Is { object })
    }
    /// True when an atom, in any alternative, compares the event's object
    /// with a reference instead of reading a characteristic.
    fn identity_only(&self) -> bool {
        self.0.iter().all(|atom| match atom {
            Atom::Is { .. } | Atom::IsNot { .. } => true,
            Atom::AnyOf { alternatives } => alternatives.iter().all(Self::identity_only),
            _ => false,
        })
    }
    /// True when the predicate reads some characteristic of the object, so
    /// the facts it was matched on are retained as evidence.
    pub fn reads_characteristics(&self) -> bool {
        !self.identity_only()
    }
}

/// The player expressions an event pattern may name: "you", each opponent
/// or each player (CR 109.5, 102.3).
fn event_players(player: &PlayerExpr) -> bool {
    matches!(
        player,
        PlayerExpr::Is {
            who: Ref::Controller {
                role: ControllerRole::AbilityController
            }
        } | PlayerExpr::EachOpponent
            | PlayerExpr::EachPlayer
    )
}

/// The atoms an event's object predicate may use. Each one is evaluated on
/// the event's frozen or current object facts without an invocation: card
/// types, supertypes, subtypes, colors, keywords, fixed power, toughness and
/// mana value comparisons, tapped status, tokens, control by an event
/// player expression and identity with the source or its host.
fn event_atom(atom: &Atom, depth: usize) -> Result<(), KirError> {
    let ok = match atom {
        Atom::Type { .. }
        | Atom::NotType { .. }
        | Atom::Legendary
        | Atom::NotLegendary
        | Atom::Basic
        | Atom::NotBasic
        | Atom::Color { .. }
        | Atom::NotColor { .. }
        | Atom::Tapped
        | Atom::Untapped
        | Atom::Token
        | Atom::NotToken
        | Atom::Keyword { .. }
        | Atom::NotKeyword { .. } => true,
        Atom::AnyType { kinds } => kinds.len() >= 2,
        Atom::Subtype { subtype } | Atom::NotSubtype { subtype } => {
            crate::cast_subtype::subtype_spelling(subtype)
        }
        Atom::ControlledBy { player } => event_players(player),
        Atom::Power { value, .. } | Atom::Toughness { value, .. } | Atom::ManaValue { value, .. } => {
            value.as_fixed().is_some()
        }
        Atom::Is { object } | Atom::IsNot { object } => {
            matches!(object, Ref::Source | Ref::AttachedTo)
        }
        Atom::AnyOf { alternatives } => {
            if depth >= 8 || alternatives.is_empty() {
                false
            } else {
                for alternative in alternatives {
                    event_pred(alternative, depth + 1)?;
                }
                true
            }
        }
        Atom::OwnedBy { .. } | Atom::Attacking | Atom::Blocking => false,
    };
    if ok {
        Ok(())
    } else {
        Err(KirError::Unaudited(format!(
            "event object atom {}",
            serde_json::to_string(atom).unwrap_or_default()
        )))
    }
}

fn event_pred(pred: &Pred, depth: usize) -> Result<(), KirError> {
    if pred.0.len() > 16 {
        return Err(KirError::Unaudited("event object predicate bound".into()));
    }
    pred.0.iter().try_for_each(|atom| event_atom(atom, depth))
}

/// CR 603.2: admit an event pattern. Its object predicate uses only the
/// atoms [`event_atom`] evaluates and its players are event player
/// expressions. `attachment` states whether the source is an Aura or
/// Equipment, the only sources with a host (CR 301.5, 303.4).
pub fn check_event(pattern: &EventPattern, attachment: bool) -> Result<(), KirError> {
    let players: &[&PlayerExpr] = match pattern {
        EventPattern::Cast { caster, .. } => &[caster],
        EventPattern::StepBegins { active, .. } => &[active],
        EventPattern::DealsDamage { recipient, .. } => &[recipient],
        _ => &[],
    };
    if let Some(player) = players.iter().find(|p| !event_players(p)) {
        return Err(KirError::Unaudited(format!("event player {player:?}")));
    }
    if let Some(object) = pattern.object() {
        event_pred(object, 0)?;
        if !attachment && mentions(object, Ref::AttachedTo) {
            return Err(KirError::Scope(
                "attached-to event object without a host".into(),
            ));
        }
        // CR 601.2i: a spell is never this permanent or its host.
        if matches!(pattern, EventPattern::Cast { .. })
            && (mentions(object, Ref::Source) || mentions(object, Ref::AttachedTo))
        {
            return Err(KirError::Unaudited("cast event identity".into()));
        }
    }
    Ok(())
}

fn mentions(pred: &Pred, object: Ref) -> bool {
    pred.0.iter().any(|atom| match atom {
        Atom::Is { object: o } | Atom::IsNot { object: o } => *o == object,
        Atom::AnyOf { alternatives } => alternatives.iter().any(|p| mentions(p, object)),
        _ => false,
    })
}

/// The intervening "if" clauses a triggered ability can check (CR 603.4).
/// Each one names one audited evaluation the rules core performs when the
/// event occurs and again as the ability resolves.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Intervening {
    /// "if you cast it": the entering source came from a spell its
    /// controller cast (CR 603.4, 400.7).
    SourceCastByController,
    /// "if you have no cards in hand" (CR 402).
    ControllerHandEmpty,
    /// "if this permanent is tapped/untapped" (CR 110.5).
    SourceTapped { tapped: bool },
    /// "if you control a creature with power N or greater" (CR 208).
    ControllerHasCreatureWithPower { minimum_power: Quantity },
    /// "if you control a [filter]" or "N or more [filter]" (CR 109.5,
    /// 608.2h): the number of battlefield permanents satisfying `filter`
    /// compared with a fixed number. The filter names "you" as their
    /// controller and may exclude the source.
    BattlefieldCount {
        filter: Pred,
        cmp: Cmp,
        value: Quantity,
    },
}

impl Intervening {
    /// The kernel condition of this intervening clause.
    pub fn condition(&self) -> Cond {
        let you = PlayerExpr::you(Context::Triggered);
        match self {
            Self::SourceCastByController => Cond::CastBy {
                object: Ref::Source,
                caster: you,
            },
            Self::ControllerHandEmpty => Cond::Compare {
                left: Amount::Count {
                    set: SetExpr::Matching {
                        zone: ZoneExpr::Hand { of: you },
                        filter: Pred::default(),
                    },
                },
                cmp: Cmp::Exactly,
                right: Amount::fixed(Quantity::zero()),
            },
            Self::SourceTapped { tapped } => Cond::Satisfies {
                object: Ref::Source,
                filter: Pred(vec![if *tapped {
                    Atom::Tapped
                } else {
                    Atom::Untapped
                }]),
            },
            Self::BattlefieldCount { filter, cmp, value }
                if *cmp == Cmp::AtLeast && *value == Quantity::from(1) =>
            {
                Cond::Exists {
                    set: SetExpr::battlefield(filter.clone()),
                }
            }
            Self::BattlefieldCount { filter, cmp, value } => Cond::Compare {
                left: Amount::Count {
                    set: SetExpr::battlefield(filter.clone()),
                },
                cmp: *cmp,
                right: Amount::Fixed {
                    value: value.clone(),
                },
            },
            Self::ControllerHasCreatureWithPower { minimum_power } => Cond::Exists {
                set: SetExpr::battlefield(Pred(vec![
                    Atom::Type {
                        kind: CardKind::Creature,
                    },
                    Atom::ControlledBy { player: you },
                    Atom::Power {
                        cmp: Cmp::AtLeast,
                        value: Amount::Fixed {
                            value: minimum_power.clone(),
                        },
                    },
                ])),
            },
        }
    }

    /// CR 603.4: the audited evaluation `condition` names, or the reason it
    /// has none. A condition is admitted only in exactly one of these forms.
    pub fn of(condition: &Cond) -> Result<Self, KirError> {
        let candidate = match condition {
            Cond::CastBy { .. } => Self::SourceCastByController,
            Cond::Compare {
                left:
                    Amount::Count {
                        set:
                            SetExpr::Matching {
                                zone: ZoneExpr::Battlefield,
                                filter,
                            },
                    },
                cmp,
                right: Amount::Fixed { value },
            } => Self::board(filter, *cmp, value)?,
            Cond::Compare { .. } => Self::ControllerHandEmpty,
            Cond::Satisfies {
                filter: Pred(atoms),
                ..
            } => Self::SourceTapped {
                tapped: atoms.first() == Some(&Atom::Tapped),
            },
            Cond::Exists {
                set: SetExpr::Matching { filter, .. },
            } => match filter.0.as_slice() {
                [
                    _,
                    _,
                    Atom::Power {
                        value: Amount::Fixed { value },
                        ..
                    },
                ] if !value.is_negative()
                    && Self::ControllerHasCreatureWithPower {
                        minimum_power: value.clone(),
                    }
                    .condition()
                        == *condition =>
                {
                    Self::ControllerHasCreatureWithPower {
                        minimum_power: value.clone(),
                    }
                }
                _ => Self::board(filter, Cmp::AtLeast, &Quantity::from(1))?,
            },
            _ => return Err(KirError::Unaudited("intervening condition".into())),
        };
        if candidate.condition() == *condition {
            Ok(candidate)
        } else {
            Err(KirError::Unaudited("intervening condition".into()))
        }
    }

    /// CR 109.5, 608.2h: a count of permanents "you" control satisfying an
    /// event-object predicate, which may exclude the source.
    fn board(filter: &Pred, cmp: Cmp, value: &Quantity) -> Result<Self, KirError> {
        let you = Atom::ControlledBy {
            player: PlayerExpr::you(Context::Triggered),
        };
        if value.is_negative()
            || !filter.0.contains(&you)
            || mentions(filter, Ref::AttachedTo)
            || filter.0.iter().any(|atom| {
                matches!(atom, Atom::Is { .. })
                    || matches!(atom, Atom::AnyOf { alternatives }
                        if alternatives.iter().any(|p| p.0.iter().any(|a| matches!(a, Atom::Is { .. }))))
            })
        {
            return Err(KirError::Unaudited("intervening battlefield count".into()));
        }
        event_pred(filter, 0)?;
        Ok(Self::BattlefieldCount {
            filter: filter.clone(),
            cmp,
            value: value.clone(),
        })
    }

    /// The trigger event this clause can be checked against. "If you cast
    /// it" needs the source's own entry; the others read only the board.
    pub fn admits(&self, pattern: &EventPattern) -> bool {
        match self {
            Self::SourceCastByController => *pattern == EventPattern::self_enters(),
            _ => true,
        }
    }
}
