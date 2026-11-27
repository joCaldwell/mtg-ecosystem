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

/// CR 115.1: the kind of stack object that announced a target.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TargetingKind {
    Spell,
    Ability,
    SpellOrAbility,
}

impl TargetingKind {
    pub fn matches(self, is_spell: bool) -> bool {
        match self {
            Self::Spell => is_spell,
            Self::Ability => !is_spell,
            Self::SpellOrAbility => true,
        }
    }
}

/// CR 603.2: "your/their Nth [filtered] spell/card each turn", or with
/// `any_player` "the Nth [filtered] spell of a turn": the event triggers only
/// when it is the `nth` matching event of the current turn, counted from the
/// turn's start whether or not the source was on the battlefield then.
/// Events of the event's player count, or of every player with `any_player`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TurnOrdinal {
    pub nth: u32,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub any_player: bool,
}

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
    /// CR 702.37e, 708.8: a face-down permanent is turned face up. It is
    /// matched on its face-up characteristics.
    TurnedFaceUp { object: Pred },
    /// CR 508.3a: a creature is declared as an attacker. It is matched on its
    /// facts immediately before the declaration.
    Attacks { attacker: Pred },
    /// CR 506.5, 508.3a: a creature is the only creature declared as an
    /// attacker in its declaration. Matched like [`Self::Attacks`].
    AttacksAlone { attacker: Pred },
    /// CR 702.149a: a creature is declared as an attacker together with at
    /// least one other creature of greater power, both read as they were
    /// before the declaration.
    AttacksWithGreaterPower { attacker: Pred },
    /// CR 702.105a: a creature is declared attacking the player with the
    /// most life or tied for most life among the players in the game, read
    /// at the declaration.
    AttacksPlayerWithMostLife { attacker: Pred },
    /// CR 509.1h, 509.3g: no blockers were declared for this attacker.
    AttacksUnblocked { attacker: Pred },
    /// CR 601.2i: a player casts a spell. The spell is matched on its frozen
    /// characteristics on the stack.
    Cast {
        caster: PlayerExpr,
        spell: Pred,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        nth: Option<TurnOrdinal>,
        /// CR 115.1, 601.2c: "a spell that targets this [permanent]": the
        /// source is among the permanents the spell targets as it becomes
        /// cast.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        targets_source: bool,
        /// CR 115.1, 601.2c: "a spell that targets a [filtered permanent]":
        /// some permanent the spell targets as it becomes cast matches.
        #[serde(default, skip_serializing_if = "Pred::is_empty")]
        target: Pred,
    },
    /// CR 115.1, 601.2c, 602.2b, 603.3d: a stack object targets a permanent.
    BecomesTarget {
        object: Pred,
        by: PlayerExpr,
        kind: TargetingKind,
    },
    /// CR 121.1–2: a player successfully draws one card.
    Draws {
        player: PlayerExpr,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        nth: Option<TurnOrdinal>,
    },
    /// CR 119.9, 120.3f: a source causes a player to gain a positive amount
    /// of life; each source's gain is its own event.
    GainsLife {
        player: PlayerExpr,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        nth: Option<TurnOrdinal>,
    },
    /// CR 701.21a: a player sacrifices a permanent, matched on the
    /// battlefield immediately before the event (CR 603.10a).
    Sacrifices { player: PlayerExpr, object: Pred },
    /// CR 701.9a: a player discards a card. `card` is matched on the
    /// discarded card in the zone it was put into (CR 603.10); with `cycled`
    /// only a card discarded to pay its own cycling or typecycling cost
    /// matches (CR 702.29a, 702.29f). "Cycles or discards" is one discard
    /// event (CR 702.29d).
    Discards {
        player: PlayerExpr,
        #[serde(default, skip_serializing_if = "Pred::is_empty")]
        card: Pred,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        cycled: bool,
        /// CR 603.2: with `cycled`, "cycle [a card] for the first time each
        /// turn" counts the event player's cycles this turn.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        nth: Option<TurnOrdinal>,
    },
    /// CR 603.6c, 603.10a: a permanent moves from the battlefield to another
    /// zone, matched on the battlefield immediately before the move.
    LeavesBattlefield {
        object: Pred,
        /// "Leaves the battlefield without dying": not put into a graveyard
        /// from the battlefield (CR 700.4).
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        without_dying: bool,
    },
    /// CR 509.3a: a creature is declared as a blocker, once per declaration
    /// however many attackers it blocks; matched as it was before.
    Blocks { blocker: Pred },
    /// CR 509.3c: an unblocked attacking creature becomes blocked by the
    /// declared blockers, matched as it was before the declaration.
    BecomesBlocked { attacker: Pred },
    /// CR 509.3b, 509.3f: a creature is declared as a blocker of an
    /// attacking creature, once per attacker it blocks; both are matched as
    /// they were before the declaration.
    BlocksCreature { blocker: Pred, attacker: Pred },
    /// CR 509.3d, 509.3f: an attacking creature becomes blocked by a
    /// declared blocker, once per blocker; both matched as for
    /// [`EventPattern::BlocksCreature`].
    BlockedByCreature { attacker: Pred, blocker: Pred },
    /// CR 120.3, 603.2c: a permanent is dealt positive damage, once per
    /// simultaneous damage event however many sources dealt it. `combat`
    /// restricts it to combat damage.
    IsDealtDamage { object: Pred, combat: bool },
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
    /// CR 702.29c: "when you discard (or cycle) this card", which triggers
    /// from the graveyard the card was put into.
    pub fn discards_source(&self) -> bool {
        matches!(self, Self::Discards { player, card, .. }
            if *card == Pred::source() && player.is_you())
    }

    /// The turn ordinal this event is counted against, if any.
    pub fn nth(&self) -> Option<TurnOrdinal> {
        match self {
            Self::Cast { nth, .. }
            | Self::Draws { nth, .. }
            | Self::GainsLife { nth, .. }
            | Self::Discards { nth, .. } => *nth,
            _ => None,
        }
    }

    /// Stable reporting vocabulary, independent of the serde representation.
    pub fn reporting_key(&self) -> &'static str {
        match self {
            Self::Enters { .. } => "enters",
            Self::Dies { .. } => "dies",
            Self::BecomesTapped { .. } => "becomes_tapped",
            Self::TurnedFaceUp { .. } => "turned_face_up",
            Self::Attacks { .. } => "attacks",
            Self::AttacksAlone { .. } => "attacks_alone",
            Self::AttacksWithGreaterPower { .. } => "attacks_with_greater_power",
            Self::AttacksPlayerWithMostLife { .. } => "attacks_player_with_most_life",
            Self::AttacksUnblocked { .. } => "attacks_unblocked",
            Self::Cast { .. } => "cast",
            Self::BecomesTarget { .. } => "becomes_target",
            Self::Draws { .. } => "draws",
            Self::GainsLife { .. } => "gains_life",
            Self::Sacrifices { .. } => "sacrifices",
            Self::Discards { .. } => "discards",
            Self::LeavesBattlefield { .. } => "leaves_battlefield",
            Self::Blocks { .. } => "blocks",
            Self::BecomesBlocked { .. } => "becomes_blocked",
            Self::BlocksCreature { .. } => "blocks_creature",
            Self::BlockedByCreature { .. } => "blocked_by_creature",
            Self::IsDealtDamage { .. } => "is_dealt_damage",
            Self::StepBegins { .. } => "step_begins",
            Self::DealsDamage { .. } => "deals_damage",
            Self::AnyOf { .. } => "any_of",
        }
    }

    pub fn object_role(&self) -> Option<EventObjectRole> {
        match self {
            Self::Enters { .. } => Some(EventObjectRole::Entered),
            Self::Attacks { .. }
            | Self::AttacksAlone { .. }
            | Self::AttacksWithGreaterPower { .. }
            | Self::AttacksPlayerWithMostLife { .. } => Some(EventObjectRole::Attacked),
            Self::BlockedByCreature { .. } => Some(EventObjectRole::Blocker),
            _ => None,
        }
    }

    /// The predicate on the event's object, when the event has one.
    pub fn object(&self) -> Option<&Pred> {
        match self {
            Self::Enters { object }
            | Self::Dies { object }
            | Self::BecomesTapped { object }
            | Self::TurnedFaceUp { object }
            | Self::BecomesTarget { object, .. }
            | Self::Sacrifices { object, .. }
            | Self::Discards { card: object, .. }
            | Self::LeavesBattlefield { object, .. }
            | Self::IsDealtDamage { object, .. } => Some(object),
            Self::Blocks { blocker } | Self::BlocksCreature { blocker, .. } => Some(blocker),
            Self::BecomesBlocked { attacker } | Self::BlockedByCreature { attacker, .. } => {
                Some(attacker)
            }
            Self::Attacks { attacker }
            | Self::AttacksAlone { attacker }
            | Self::AttacksWithGreaterPower { attacker }
            | Self::AttacksPlayerWithMostLife { attacker }
            | Self::AttacksUnblocked { attacker } => Some(attacker),
            Self::Cast { spell, .. } => Some(spell),
            Self::DealsDamage { source, .. } => Some(source),
            Self::StepBegins { .. }
            | Self::Draws { .. }
            | Self::GainsLife { .. }
            | Self::AnyOf { .. } => None,
        }
    }

    /// "At the beginning of your upkeep" (CR 503.1a).
    pub fn your_upkeep() -> Self {
        Self::StepBegins {
            step: crate::BeginningStep::Upkeep,
            active: PlayerExpr::you(Context::Triggered),
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
    /// "When this permanent is turned face up" (CR 702.37e, 708.8).
    pub fn self_turned_face_up() -> Self {
        Self::TurnedFaceUp {
            object: Pred::source(),
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
            | (
                EventPattern::Enters { object },
                EventPattern::LeavesBattlefield { object: other, .. },
            ) if *object == Pred::source() && *other == Pred::source() => Ok(()),
            // CR 506.4, 509.1a: an attacking creature is never declared as a
            // blocker, so one declaration is either the attack or the block.
            (EventPattern::Attacks { attacker }, EventPattern::Blocks { blocker })
                if *attacker == Pred::source() && *blocker == Pred::source() =>
            {
                Ok(())
            }
            // CR 119.9, 603.6a: a life gain is never an entry.
            (EventPattern::Enters { object }, EventPattern::GainsLife { nth: None, .. })
                if *object == Pred::source() =>
            {
                check_event(second, attachment)
            }
            // CR 509.1a: a blocking creature is never also an attacker
            // becoming blocked, so one declaration cannot match both.
            (EventPattern::Blocks { blocker }, EventPattern::BecomesBlocked { attacker })
                if *blocker == Pred::source() && *attacker == Pred::source() =>
            {
                Ok(())
            }
            // CR 509.1a: in one blocking pair the source is either the
            // blocker or the attacker, never both.
            (
                EventPattern::BlocksCreature { blocker, .. },
                EventPattern::BlockedByCreature { attacker, .. },
            ) if *blocker == Pred::source() && *attacker == Pred::source() => {
                check_event(first, attachment)?;
                check_event(second, attachment)
            }
            (EventPattern::Enters { .. }, EventPattern::Cast { .. }) => {
                check_event(first, attachment)?;
                check_event(second, attachment)
            }
            _ => Err(KirError::Unaudited("event alternatives".into())),
        };
    }
    let players: &[&PlayerExpr] = match pattern {
        EventPattern::Cast { caster, .. } => &[caster],
        EventPattern::StepBegins { active, .. } => &[active],
        EventPattern::Draws { player, .. }
        | EventPattern::GainsLife { player, .. }
        | EventPattern::Sacrifices { player, .. }
        | EventPattern::Discards { player, .. } => &[player],
        EventPattern::BecomesTarget { by, .. } => &[by],
        EventPattern::DealsDamage { recipient, .. } => &[recipient],
        _ => &[],
    };
    if let Some(player) = players.iter().find(|p| !event_players(p)) {
        return Err(KirError::Unaudited(format!("event player {player:?}")));
    }
    // CR 603.2: the first four ordinals are printed; "the Nth ... of a
    // turn" counts every caster's spells, so it names each caster.
    if let Some(nth) = pattern.nth()
        && (!(1..=4).contains(&nth.nth)
            || (nth.any_player
                && !matches!(
                    pattern,
                    EventPattern::Cast {
                        caster: PlayerExpr::EachPlayer,
                        ..
                    }
                )))
    {
        return Err(KirError::Unaudited("event turn ordinal".into()));
    }
    // The ledger counts every cycle, so an ordinal discard is an unfiltered
    // cycle ("another card" excludes nothing a battlefield source cycles).
    if let EventPattern::Discards {
        nth: Some(ordinal),
        cycled,
        card,
        ..
    } = pattern
        && (!*cycled
            || ordinal.any_player
            || card.reads_characteristics()
            || card.is_ref(Ref::Source))
    {
        return Err(KirError::Unaudited("discard turn ordinal".into()));
    }
    // CR 601.2c: a cast spell's targeted permanent is read as the spell
    // becomes cast; "this permanent" is `targets_source`.
    if let EventPattern::Cast { target, .. } = pattern
        && !target.is_empty()
    {
        event_pred(target, 0)?;
        if mentions(target, Ref::Source) || mentions(target, Ref::AttachedTo) {
            return Err(KirError::Unaudited("cast target identity".into()));
        }
    }
    // CR 509.3f: the other creature of a blocking pair is read as it was
    // declared; it is never the source or its host, and has no status here.
    if let EventPattern::BlocksCreature {
        attacker: other, ..
    }
    | EventPattern::BlockedByCreature { blocker: other, .. } = pattern
    {
        event_pred(other, 0)?;
        if mentions(other, Ref::Source)
            || mentions(other, Ref::AttachedTo)
            || any_atom(other, &|a| matches!(a, Atom::Tapped | Atom::Untapped))
        {
            return Err(KirError::Unaudited("blocking pair creature".into()));
        }
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
            EventPattern::Cast { .. }
                | EventPattern::Attacks { .. }
                | EventPattern::AttacksAlone { .. }
                | EventPattern::AttacksWithGreaterPower { .. }
                | EventPattern::AttacksPlayerWithMostLife { .. }
                | EventPattern::Blocks { .. }
                | EventPattern::BecomesBlocked { .. }
                | EventPattern::BlocksCreature { .. }
                | EventPattern::BlockedByCreature { .. }
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
        // CR 701.9a, 109.1: a discarded card in a graveyard or exile has
        // printed characteristics only: no controller, status or token
        // facts. It is this object only for "when you discard this card",
        // which functions from the graveyard (CR 702.29c); otherwise only
        // "another" names the source.
        if matches!(pattern, EventPattern::Discards { .. })
            && *object != Pred::source()
            && any_atom(object, &|a| {
                !matches!(
                    a,
                    Atom::Type { .. }
                        | Atom::NotType { .. }
                        | Atom::AnyType { .. }
                        | Atom::Subtype { .. }
                        | Atom::NotSubtype { .. }
                        | Atom::Color { .. }
                        | Atom::NotColor { .. }
                        | Atom::Legendary
                        | Atom::NotLegendary
                        | Atom::Basic
                        | Atom::NotBasic
                        | Atom::ManaValue { .. }
                        | Atom::AnyOf { .. }
                        | Atom::IsNot {
                            object: Ref::Source
                        }
                )
            })
        {
            return Err(KirError::Unaudited("discarded card atom".into()));
        }
        // CR 701.26a: the collector observes only the source's own tap.
        if matches!(pattern, EventPattern::BecomesTapped { .. }) && *object != Pred::source() {
            return Err(KirError::Unaudited("becomes-tapped object".into()));
        }
        // CR 702.37e: the collector observes only the source turning face up.
        if matches!(pattern, EventPattern::TurnedFaceUp { .. }) && *object != Pred::source() {
            return Err(KirError::Unaudited("turned-face-up object".into()));
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

/// Exact measurements from the current turn (CR 601.2i, 508.1, 700.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TurnFact {
    Attacked,
    CreatureDied,
    CreatureDiedUnderYourControl,
    PermanentLeftUnderYourControl,
    SpellsCast,
    NoncreatureSpellsCast,
    SpellsCastFromHand,
    CardsDrawn,
    LifeGained,
    /// CR 111.2: tokens this player created this turn.
    TokensCreated,
    /// CR 601.2i: instant and sorcery spells this player cast this turn.
    InstantOrSorcerySpellsCast,
}

/// The intervening "if" clauses a triggered ability can check (CR 603.4).
/// Each one names one audited evaluation the rules core performs when the
/// event occurs and again as the ability resolves.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Intervening {
    TurnHistory {
        fact: TurnFact,
        cmp: Cmp,
        value: Quantity,
    },
    /// "if you cast it": the entering source came from a spell its
    /// controller cast (CR 603.4, 400.7).
    SourceCastByController,
    /// "if its evoke cost was paid": the entering source came from a spell
    /// whose caster announced and paid this alternative cost (CR 702.74a,
    /// 603.4, 400.7).
    SourceCostPaid {
        identity: String,
    },
    /// Echo's "if this permanent came under your control since the
    /// beginning of your last upkeep" (CR 702.30a, 603.4).
    SourceControlledSinceLastUpkeep,
    /// "if you have no cards in hand" (CR 402).
    ControllerHandEmpty,
    /// CR 603.4, 109.5: a fixed comparison of information about "you".
    ControllerComparison {
        measure: ControllerMeasure,
        cmp: Cmp,
        value: Quantity,
    },
    /// "if this permanent is tapped/untapped" (CR 110.5).
    SourceTapped {
        tapped: bool,
    },
    ControlsOwnCommander,
    OpponentControlsMore {
        kind: CardKind,
    },
    /// "if you control a creature with power N or greater" (CR 208).
    ControllerHasCreatureWithPower {
        minimum_power: Quantity,
    },
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
            Self::TurnHistory { fact, cmp, value } => Cond::TurnHistory {
                fact: *fact,
                cmp: *cmp,
                value: value.clone(),
            },
            Self::ControlsOwnCommander => Cond::ControlsOwnCommander {},
            Self::OpponentControlsMore { kind } => Cond::OpponentControlsMore { kind: *kind },
            Self::SourceCastByController => Cond::CastBy {
                object: Ref::Source,
                caster: you,
            },
            Self::SourceCostPaid { identity } => Cond::CostPaid {
                identity: identity.clone(),
            },
            Self::SourceControlledSinceLastUpkeep => Cond::ControlledSinceLastUpkeep {},
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
            Cond::TurnHistory { fact, cmp, value } if !value.is_negative() => Self::TurnHistory {
                fact: *fact,
                cmp: *cmp,
                value: value.clone(),
            },
            Cond::CastBy { .. } => Self::SourceCastByController,
            Cond::ControlledSinceLastUpkeep {} => Self::SourceControlledSinceLastUpkeep,
            // CR 702.74a: the one reviewed entry cost an intervening clause reads.
            Cond::CostPaid { identity } if identity == crate::EVOKE_COST => Self::SourceCostPaid {
                identity: identity.clone(),
            },
            Cond::ControlsOwnCommander {} => Self::ControlsOwnCommander,
            Cond::OpponentControlsMore {
                kind: kind @ (CardKind::Land | CardKind::Creature),
            } => Self::OpponentControlsMore { kind: *kind },
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
            Self::SourceCastByController | Self::SourceCostPaid { .. } => {
                *pattern == EventPattern::self_enters()
            }
            Self::SourceControlledSinceLastUpkeep => *pattern == EventPattern::your_upkeep(),
            _ if matches!(pattern, EventPattern::BecomesTarget { .. }) => false,
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
