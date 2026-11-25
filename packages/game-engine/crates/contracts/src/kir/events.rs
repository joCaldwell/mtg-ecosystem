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
    /// CR 603.2c: one ability with two disjoint trigger events.
    AnyOf {
        first: Box<EventPattern>,
        second: Box<EventPattern>,
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
            Self::AnyOf { .. } => "any_of",
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
            Self::StepBegins { .. } | Self::AnyOf { .. } => None,
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
        Atom::Power { value, .. }
        | Atom::Toughness { value, .. }
        | Atom::ManaValue { value, .. } => value.as_fixed().is_some(),
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
    if let EventPattern::AnyOf { first, second } = pattern {
        // Each branch names the source and neither binds an event operand.
        // A single occurrence cannot be both an entry and a death or attack.
        return match (&**first, &**second) {
            (EventPattern::Enters { object }, EventPattern::Dies { object: other })
            | (EventPattern::Enters { object }, EventPattern::Attacks { attacker: other })
                if *object == Pred::source() && *other == Pred::source() =>
            {
                Ok(())
            }
            _ => Err(KirError::Unaudited("event alternatives".into())),
        };
    }
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
        // CR 110.5d: a spell has no tapped status. CR 508.1f taps an
        // attacker after the facts an attack event is matched on, so a
        // status read there would not be the declared attacker's.
        if matches!(
            pattern,
            EventPattern::Cast { .. } | EventPattern::Attacks { .. }
        ) && any_atom(object, &|a| matches!(a, Atom::Tapped | Atom::Untapped))
        {
            return Err(KirError::Unaudited("event status atom".into()));
        }
        // A snapshot records no historical host, so recovery can prove
        // that an object was the host but never that it was not.
        if any_atom(object, &|a| {
            matches!(
                a,
                Atom::IsNot {
                    object: Ref::AttachedTo
                }
            )
        }) {
            return Err(KirError::Unaudited("event object excludes the host".into()));
        }
        // CR 701.26a: the collector observes only the source's own tap.
        if matches!(pattern, EventPattern::BecomesTapped { .. }) && *object != Pred::source() {
            return Err(KirError::Unaudited("becomes-tapped object".into()));
        }
    }
    Ok(())
}

/// True when some atom, at any depth of alternatives, satisfies `test`.
fn any_atom(pred: &Pred, test: &impl Fn(&Atom) -> bool) -> bool {
    pred.0.iter().any(|atom| {
        test(atom)
            || matches!(atom, Atom::AnyOf { alternatives }
                if alternatives.iter().any(|p| any_atom(p, test)))
    })
}

fn mentions(pred: &Pred, object: Ref) -> bool {
    any_atom(
        pred,
        &|atom| matches!(atom, Atom::Is { object: o } | Atom::IsNot { object: o } if *o == object),
    )
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
    /// CR 603.4, 109.5: a fixed comparison of information about "you".
    ControllerComparison {
        measure: ControllerMeasure,
        cmp: Cmp,
        value: Quantity,
    },
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ControllerMeasure {
    Life,
    HandCount,
    GraveyardCount(Pred),
}

impl ControllerMeasure {
    /// CR 110.4: the permanent card types present in the admitted catalog.
    pub fn permanent_cards() -> Pred {
        Pred(vec![Atom::AnyType {
            kinds: vec![
                CardKind::Land,
                CardKind::Creature,
                CardKind::Enchantment,
                CardKind::Artifact,
            ],
        }])
    }

    fn amount(&self) -> Amount {
        let of = PlayerExpr::you(Context::Triggered);
        match self {
            Self::Life => Amount::LifeOf { player: of },
            Self::HandCount => Amount::Count {
                set: SetExpr::Matching {
                    zone: ZoneExpr::Hand { of },
                    filter: Pred::default(),
                },
            },
            Self::GraveyardCount(filter) => Amount::Count {
                set: SetExpr::Matching {
                    zone: ZoneExpr::Graveyard { of },
                    filter: filter.clone(),
                },
            },
        }
    }

    fn of(amount: &Amount) -> Option<Self> {
        let you = PlayerExpr::you(Context::Triggered);
        match amount {
            Amount::LifeOf { player } if *player == you => Some(Self::Life),
            Amount::Count {
                set:
                    SetExpr::Matching {
                        zone: ZoneExpr::Hand { of },
                        filter,
                    },
            } if *of == you && filter.0.is_empty() => Some(Self::HandCount),
            Amount::Count {
                set:
                    SetExpr::Matching {
                        zone: ZoneExpr::Graveyard { of },
                        filter,
                    },
            } if *of == you
                && (filter.0.is_empty()
                    || *filter == Self::permanent_cards()
                    || matches!(filter.0.as_slice(), [Atom::Type { .. }])) =>
            {
                Some(Self::GraveyardCount(filter.clone()))
            }
            _ => None,
        }
    }
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
            Self::ControllerComparison {
                measure,
                cmp,
                value,
            } => Cond::Compare {
                left: measure.amount(),
                cmp: *cmp,
                right: Amount::fixed(value.clone()),
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
            Cond::Compare {
                left,
                cmp,
                right: Amount::Fixed { value },
            } => match ControllerMeasure::of(left) {
                Some(ControllerMeasure::HandCount)
                    if *cmp == Cmp::Exactly && *value == Quantity::zero() =>
                {
                    Self::ControllerHandEmpty
                }
                Some(measure) if !value.is_negative() => Self::ControllerComparison {
                    measure,
                    cmp: *cmp,
                    value: value.clone(),
                },
                _ => {
                    return Err(KirError::Unaudited(
                        "intervening controller comparison".into(),
                    ));
                }
            },
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
            || any_atom(filter, &|atom| matches!(atom, Atom::Is { .. }))
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
            Self::ControllerComparison { .. } => match pattern {
                EventPattern::Enters { .. }
                | EventPattern::Attacks { .. }
                | EventPattern::StepBegins { .. } => true,
                EventPattern::AnyOf { first, second } => self.admits(first) && self.admits(second),
                _ => false,
            },
            _ => true,
        }
    }
}
