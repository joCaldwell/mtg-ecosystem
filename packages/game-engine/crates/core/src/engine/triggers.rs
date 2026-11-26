use super::*;
use crate::query::{self, ObjectFacts, Tri};
use mtg_contracts::kir::{self, Atom, EventPattern, PlayerExpr, Pred, Ref};
use std::borrow::Cow;

mod targeting;
pub(super) use targeting::TargetTriggerDraft;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TriggerId(pub u64);
pub fn is_zero(value: &u64) -> bool {
    *value == 0
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PendingTrigger {
    /// CR 113.7a, 712.18: the face whose ability triggered.
    #[serde(default, skip_serializing_if = "CardFace::is_front")]
    pub source_face: CardFace,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    death_controller: Option<PlayerId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub match_condition: Option<InterveningCheck>,
    pub id: TriggerId,
    pub source: ObjectId,
    pub controller: PlayerId,
    pub ability: EffectiveTriggerAbility,
    pub source_facts: ObjectBefore,
    pub(super) source_handles: Vec<publish::StoredObjectHandle>,
    event: TriggerOccurrenceId,
    pub witness: TriggerEventWitness,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TriggerPlacement {
    pub(super) resume: FrameLink,
    cleanup: bool,
    changed: bool,
    remaining: Vec<PlayerId>,
    before: u64,
    order: Vec<TriggerId>,
}
/// Batch occurrences and completed casts have independent identity domains.
/// The untagged batch alternative preserves the existing {batch,path} bytes;
/// cast IDs name the real SpellCast event, not an invented event-group slot.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TriggerOccurrenceId {
    Batch(OccurrenceId),
    Cast(CastOccurrenceId),
    Target(TargetOccurrenceId),
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CastOccurrenceId {
    pub cast: StackId,
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TargetOccurrenceId {
    pub targeting: StackId,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum TriggerMovement {
    Entry,
    Death,
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct TriggerMember<'a> {
    source: ObjectId,
    face: CardFace,
    ability: EffectiveTriggerAbility,
    event: &'a TriggerOccurrenceId,
    subject: Option<ObjectId>,
    damage: Option<DamageId>,
}

#[derive(Default)]
struct TriggerOccurrences<'a> {
    members: BTreeSet<TriggerMember<'a>>,
    movements: BTreeSet<(
        ObjectId,
        CardFace,
        EffectiveTriggerAbility,
        ObjectId,
        TriggerMovement,
    )>,
}

/// Semantic per-turn player ledger, independent of observers and tracing:
/// successful casts, draws, life gains, attacks and departures this turn, whether
/// or not any observer was on the battlefield (CR 603.2). Original physical
/// seats never compact on departure (CR 601.2i/800.4).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TurnCastHistory {
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub attacked: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub creature_died: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub permanent_left: bool,
    #[serde(default, skip_serializing_if = "TurnCastHistory::is_zero")]
    pub hand_casts: u32,
    #[serde(default, skip_serializing_if = "TurnCastHistory::quantity_is_zero")]
    pub life_gained: Quantity,
    pub any_cast: bool,
    pub exile_cast: bool,
    pub mana_value: Quantity,
    /// CR 601.2i: each spell this player cast this turn, frozen on the stack
    /// as it became cast, in order ("your Nth [filtered] spell").
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub spells: Vec<ObjectBefore>,
    /// CR 121.2: cards this player drew this turn.
    #[serde(default, skip_serializing_if = "TurnCastHistory::is_zero")]
    pub draws: u32,
    /// CR 119.9: life-gain events of this player this turn.
    #[serde(default, skip_serializing_if = "TurnCastHistory::is_zero")]
    pub life_gains: u32,
}
impl TurnCastHistory {
    fn quantity_is_zero(value: &Quantity) -> bool {
        *value == Quantity::zero()
    }
    fn is_zero(value: &u32) -> bool {
        *value == 0
    }
}
/// CR 400.7: "This ability triggers only once each turn" was used this turn
/// by this incarnation's intrinsic ability on this face.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TriggerUse {
    pub source: ObjectId,
    #[serde(default, skip_serializing_if = "CardFace::is_front")]
    pub face: CardFace,
    pub ability: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CastHistoryFacts {
    pub turn_number: u64,
    pub first_overall: bool,
    pub first_from_exile: bool,
}
testkit_fields! {
    /// Frozen CR 601.2i event facts. The caster is not the active player.
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct CastOccurrence {
        pub history: CastHistoryFacts,
        pub x: Option<Quantity>,
        pub stack_id: StackId,
        pub caster: PlayerId,
        pub cast_from: Zone,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        pub(crate) priority_paid_exile: bool,
        pub spell: ObjectBefore,
        /// CR 603.2: the spell's count among the matching spells of the
        /// observer's turn ordinal, recorded only for such an observer.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub nth: Option<u32>,
        /// CR 601.2c: the permanents the spell targets as it becomes cast.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        pub targets: Vec<ObjectId>,
    }
}
/// The event subject's resolving binding (CR 400.7, 608.2h).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservedObject {
    controller_at_match: PlayerId,
    pub(super) last_known: Box<ObjectBefore>,
    pub(super) handles: Vec<publish::StoredObjectHandle>,
}

/// Actual trigger-event evidence, independent of whether the event has an object.
/// Source/controller facts are frozen separately at collection (CR 603.3a).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case", deny_unknown_fields)]
pub enum TriggerEventWitness {
    /// CR 603.6a, 603.10: an entry whose pattern read the entering
    /// permanent's characteristics, with the facts it was matched on.
    Entered {
        movement: Option<MoveOutcome>,
        entrant: Box<ObjectBefore>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        binding: Option<ObservedObject>,
    },
    /// Immutable damage-time facts, independent of departure-refreshed source
    /// LKI. None identifies the genuine legacy CombatDamage leaf; Some names
    /// the original frozen transaction, never the current combat state.
    DamageDealt {
        kind: DamageKind,
        transaction: Option<DamageTransactionId>,
        damage: Box<ActualDamage>,
        /// CR 109.2/603.10: creature filters inspect the exact permanent at
        /// the damage event, separately from assigned damage-source LKI.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        match_source: Option<Box<ObjectBefore>>,
    },
    CreatureAttacked {
        attacker: Box<ObjectBefore>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        binding: Option<ObservedObject>,
        defender_player_id: PlayerId,
    },
    AttackerUnblocked {
        attacker: Box<ObjectBefore>,
        defender: PlayerId,
        controller_at_match: PlayerId,
    },
    BecameTarget {
        target: Box<ObjectBefore>,
        target_controller_live: bool,
        actor: PlayerId,
        is_spell: bool,
        controller_at_match: PlayerId,
    },
    CardDrawn {
        player: PlayerId,
        controller_at_match: PlayerId,
        /// The draw's count this turn, recorded for a turn-ordinal observer.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        nth: Option<u32>,
    },
    /// CR 119.9: one source's positive life gain for one player.
    LifeGained {
        player: PlayerId,
        amount: Quantity,
        controller_at_match: PlayerId,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        nth: Option<u32>,
    },
    /// CR 701.9a: a completed discard from hand to graveyard.
    Discarded {
        player: PlayerId,
        movement: MoveOutcome,
        controller_at_match: PlayerId,
    },
    /// CR 509.3a: a declared blocker, as it was before the declaration.
    Blocked {
        blocker: Box<ObjectBefore>,
        controller_at_match: PlayerId,
    },
    /// CR 509.3c: an attacker that became blocked, as it was before.
    BecameBlocked {
        attacker: Box<ObjectBefore>,
        controller_at_match: PlayerId,
    },
    /// CR 120.3, 603.2c: a permanent dealt positive damage by one
    /// simultaneous damage event, as it was when the damage was dealt.
    PermanentDamaged {
        kind: DamageKind,
        recipient: Box<ObjectBefore>,
        controller_at_match: PlayerId,
    },
    /// CR 701.21a, 603.10a: a sacrificed permanent, with its battlefield
    /// facts immediately before the move.
    Sacrificed {
        player: PlayerId,
        object: Box<ObjectBefore>,
        controller_at_match: PlayerId,
    },
    SpellCast {
        cast: Box<CastOccurrence>,
    },
    Moved {
        movement: MoveOutcome,
    },
    TokenEntered {
        object: ObjectId,
    },
    Tapped {
        object: ObjectId,
        controller: PlayerId,
    },
    /// CR 702.37e, 708.8: the source was turned face up under `controller`.
    TurnedFaceUp {
        object: ObjectId,
        controller: PlayerId,
    },
    PermanentLeftGame {
        object: ObjectId,
        owner: PlayerId,
    },
    ResolutionPaid {
        player: PlayerId,
        cost: ManaCost,
    },
    StepBegan {
        turn: TurnState,
        owner: PlayerId,
        active: Option<PlayerId>,
    },
}
impl TriggerEventWitness {
    pub(super) fn observed_object(&self) -> Option<&ObservedObject> {
        match self {
            Self::Entered { binding, .. } | Self::CreatureAttacked { binding, .. } => {
                binding.as_ref()
            }
            _ => None,
        }
    }

    pub(super) fn refresh_observed_object(&mut self, facts: &ObjectBefore) {
        match self {
            Self::Entered { binding, .. } | Self::CreatureAttacked { binding, .. } => {
                if let Some(binding) = binding
                    && binding.last_known.object == facts.object
                {
                    *binding.last_known = facts.clone();
                }
            }
            _ => {}
        }
    }

    pub fn subject(&self) -> Option<ObjectId> {
        match self {
            Self::Entered { entrant, .. } => Some(entrant.object),
            Self::BecameTarget { target, .. } => Some(target.object),
            Self::Moved { movement: m } => Some(if m.destination == Zone::Battlefield {
                m.successor
            } else {
                m.previous
            }),
            Self::TokenEntered { object }
            | Self::Tapped { object, .. }
            | Self::TurnedFaceUp { object, .. }
            | Self::PermanentLeftGame { object, .. } => Some(*object),
            Self::CreatureAttacked { attacker, .. } | Self::AttackerUnblocked { attacker, .. } => {
                Some(attacker.object)
            }
            Self::DamageDealt { damage, .. } => Some(damage.source.object),
            Self::Sacrificed { object, .. } => Some(object.object),
            Self::Blocked { blocker: o, .. }
            | Self::BecameBlocked { attacker: o, .. }
            | Self::PermanentDamaged { recipient: o, .. } => Some(o.object),
            Self::Discarded { movement, .. } => Some(movement.previous),
            Self::LifeGained { .. }
            | Self::StepBegan { .. }
            | Self::SpellCast { .. }
            | Self::CardDrawn { .. }
            | Self::ResolutionPaid { .. } => None,
        }
    }
    pub(super) fn event_player(&self) -> Option<PlayerId> {
        match self {
            Self::StepBegan { active, .. } => *active,
            Self::CardDrawn { player, .. }
            | Self::LifeGained { player, .. }
            | Self::Discarded { player, .. }
            | Self::Sacrificed { player, .. } => Some(*player),
            Self::AttackerUnblocked { defender, .. } => Some(*defender),
            Self::SpellCast { cast } => Some(cast.caster),
            _ => None,
        }
    }
    /// CR 603.10a: leaves-the-battlefield events look back in time.
    fn looks_back(&self) -> bool {
        self.leaves() || matches!(self, Self::Sacrificed { .. })
    }
    /// CR 603.6c: a zone change or a phased-in permanent leaving the game.
    fn leaves(&self) -> bool {
        matches!(self, Self::PermanentLeftGame { .. })
            || matches!(self, Self::Moved { movement: m } if m.from == Zone::Battlefield && m.destination != Zone::Battlefield)
    }
    fn dies(&self) -> bool {
        matches!(self, Self::Moved { movement: m } if m.from == Zone::Battlefield && matches!(m.destination, Zone::Graveyard { .. }))
    }
    fn enters(&self) -> bool {
        matches!(self, Self::TokenEntered { .. } | Self::Entered { .. })
            || matches!(self, Self::Moved { movement: m } if m.destination == Zone::Battlefield && m.from != Zone::Battlefield)
    }

    // CR 603.2c: the audited mixed alternatives cannot describe the same
    // occurrence. Collection and restore both use the actual event kind.
    fn branch<'a>(&self, pattern: &'a EventPattern) -> Option<&'a EventPattern> {
        let EventPattern::AnyOf { first, second } = pattern else {
            return Some(pattern);
        };
        if self.enters() {
            return Some(first);
        }
        match (&**second, self) {
            (EventPattern::Dies { .. }, witness) if witness.dies() => Some(second),
            (EventPattern::Attacks { .. }, Self::CreatureAttacked { .. })
            | (EventPattern::Cast { .. }, Self::SpellCast { .. })
            | (EventPattern::BecomesBlocked { .. }, Self::BecameBlocked { .. }) => Some(second),
            (_, Self::Blocked { .. }) if matches!(**first, EventPattern::Blocks { .. }) => {
                Some(first)
            }
            _ => None,
        }
    }
}
/// CR 603.10: the facts an event froze for its object, read as the object
/// was at that moment: the entering permanent, the declared attacker, the
/// cast spell or the damage source's battlefield appearance.
struct Frozen<'a> {
    facts: &'a ObjectBefore,
    token: bool,
    live: bool,
}

impl ObjectFacts for Frozen<'_> {
    fn zone(&self) -> Zone {
        self.facts.zone
    }
    fn owner(&self) -> Option<PlayerId> {
        Some(self.facts.owner)
    }
    fn controller(&self) -> PlayerId {
        self.facts.controller
    }
    fn phased_out(&self) -> bool {
        false
    }
    fn tapped(&self) -> bool {
        self.facts.tapped
    }
    fn is_token(&self) -> bool {
        self.token
    }
    fn copiable(&self) -> Option<Cow<'_, Characteristics>> {
        Some(Cow::Borrowed(&self.facts.characteristics))
    }
    fn structural(&self) -> Option<Cow<'_, Characteristics>> {
        Some(Cow::Borrowed(&self.facts.characteristics))
    }
    fn current(&self) -> Option<Cow<'_, Characteristics>> {
        Some(Cow::Borrowed(&self.facts.characteristics))
    }
    fn derived(&self) -> Option<Cow<'_, Characteristics>> {
        Some(Cow::Borrowed(&self.facts.characteristics))
    }
    fn has_keyword(&self, keyword: KeywordAbility) -> Tri {
        self.facts.keywords.contains(&keyword).into()
    }
    fn controller_is_live(&self) -> Tri {
        self.live.into()
    }
    fn attacking(&self) -> Tri {
        Tri::Unknown
    }
    fn blocking(&self) -> Tri {
        Tri::Unknown
    }
}

/// CR 603.2: whether an event's object `subject` satisfies an event
/// predicate for an ability controlled by `you`. `identity` resolves the
/// source and its host; `facts` are the object's facts at the event, absent
/// when only identity is compared.
pub(super) fn holds<F: ObjectFacts>(
    pred: &Pred,
    facts: Option<&F>,
    you: PlayerId,
    subject: ObjectId,
    identity: &impl Fn(&Ref) -> Option<ObjectId>,
) -> bool {
    pred.0.iter().all(|atom| match atom {
        Atom::Is { object } => identity(object) == Some(subject),
        Atom::IsNot { object } => identity(object) != Some(subject),
        Atom::AnyOf { alternatives } => alternatives
            .iter()
            .any(|p| holds(p, facts, you, subject, identity)),
        _ => facts.is_some_and(|f| query::matches_atom(f, you, atom) == Tri::Yes),
    })
}

/// The identity atoms alone: what a snapshot can recheck for an event that
/// retained no facts for its object.
fn identity_holds(
    pred: &Pred,
    subject: ObjectId,
    identity: &impl Fn(&Ref) -> Option<ObjectId>,
) -> bool {
    pred.0.iter().all(|atom| match atom {
        Atom::Is { object } => identity(object) == Some(subject),
        Atom::IsNot { object } => identity(object) != Some(subject),
        Atom::AnyOf { alternatives } => alternatives
            .iter()
            .any(|p| identity_holds(p, subject, identity)),
        _ => true,
    })
}

/// CR 109.5, 102.3, 800.4j: whether the event's player is in an event
/// player expression. Each player also matches a turn without an active
/// player; "you" and "an opponent" need that player.
fn player_in(expr: &PlayerExpr, player: Option<PlayerId>, you: PlayerId) -> bool {
    match expr {
        PlayerExpr::EachPlayer => true,
        PlayerExpr::EachOpponent => player.is_some_and(|p| p != you),
        PlayerExpr::Is {
            who: Ref::Controller { .. },
        } => player == Some(you),
        _ => false,
    }
}

/// CR 603.2: an ordinal pattern's witness records the count it matched at,
/// and only such a witness does. Replay proves the count itself.
fn ordinal_evidence(pattern: &EventPattern, nth: Option<u32>) -> bool {
    pattern.nth().map(|ordinal| ordinal.nth) == nth
}

fn step_matches(step: BeginningStep, turn: &TurnState) -> bool {
    matches!(
        (step, turn.step),
        (BeginningStep::Upkeep, TurnStep::Upkeep)
            | (BeginningStep::Draw, TurnStep::Draw)
            | (BeginningStep::PrecombatMain, TurnStep::PrecombatMain)
            | (BeginningStep::End, TurnStep::End)
            | (BeginningStep::BeginningOfCombat, TurnStep::BeginningCombat)
            | (BeginningStep::EndOfCombat, TurnStep::EndCombat)
            | (BeginningStep::PostcombatMain, TurnStep::PostcombatMain)
    )
}

// CR 109.5/603.3a/603.10: an ordinary trigger observes its source's
// controller at the occurrence, independently of the controller of the damage
// invocation and of assignment-time source facts. A pattern that reads the
// source's characteristics reads its live, nontoken battlefield appearance;
// one that only compares identity reads the actual damage source.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
#[cfg(feature = "testkit")]
pub fn damage_matches(
    condition: &TriggerCondition,
    source: ObjectId,
    controller: PlayerId,
    kind: DamageKind,
    damage: &ActualDamage,
    match_source: Option<&ObjectBefore>,
) -> bool {
    let EventPattern::DealsDamage {
        source: pred,
        combat,
        recipient,
    } = condition.pattern()
    else {
        return false;
    };
    let identity = |r: &Ref| (*r == Ref::Source).then_some(source);
    damage.amount > Quantity::zero()
        && (!*combat || kind == DamageKind::Combat)
        && matches!(damage.recipient, DamageRecipient::Player { player }
            if player_in(recipient, Some(player), controller))
        && if pred.reads_characteristics() {
            match_source.is_some_and(|appearance| {
                appearance.object == damage.source.object
                    && appearance.zone == Zone::Battlefield
                    && holds(
                        pred,
                        Some(&Frozen {
                            facts: appearance,
                            token: false,
                            live: true,
                        }),
                        controller,
                        appearance.object,
                        &identity,
                    )
            })
        } else {
            match_source.is_none() && identity_holds(pred, damage.source.object, &identity)
        }
}

/// CR 603.10a: a trigger on an opponent's creature dying freezes that
/// creature's last controller with the invocation.
pub(crate) fn binds_death_controller(ability: &TriggeredAbility) -> bool {
    *ability.condition.pattern() == EventPattern::opponent_creature_dies()
}

/// The entry evidence of an entering permanent whose characteristics a
/// pattern read (CR 603.6a, 603.10).
fn binds_observed_object(ability: &TriggeredAbility) -> bool {
    ability.programs().any(|program| {
        program.instructions.iter().any(|i| {
            matches!(
                i.verb(),
                Some(
                    kir::Verb::Damage {
                        source: Ref::EventObject { .. },
                        ..
                    } | kir::Verb::PutCounters {
                        on: kir::SetExpr::One {
                            object: Ref::EventObject { .. }
                        },
                        ..
                    }
                )
            )
        })
    })
}

impl StackKind {
    pub(super) fn observed_object(&self) -> Result<&ObservedObject, String> {
        match self {
            Self::Triggered { witness, .. } => witness
                .observed_object()
                .ok_or("missing event object binding".into()),
            _ => Err("event object outside a triggered invocation".into()),
        }
    }
}

fn entry_witness(
    entrant: &ObjectBefore,
    witness: &TriggerEventWitness,
) -> Result<TriggerEventWitness, String> {
    let movement = match witness {
        TriggerEventWitness::Moved { movement } => Some(movement.clone()),
        TriggerEventWitness::TokenEntered { .. } => None,
        _ => return Err("missing actual entry movement".into()),
    };
    Ok(TriggerEventWitness::Entered {
        movement,
        entrant: Box::new(entrant.clone()),
        binding: None,
    })
}

pub(super) fn movement(outcome: &EventOutcome) -> Option<&MoveOutcome> {
    match outcome {
        EventOutcome::EnteredAttached { movement, .. }
        | EventOutcome::Moved { movement }
        | EventOutcome::Sacrificed { movement, .. }
        | EventOutcome::CardDiscarded { movement, .. }
        | EventOutcome::StateBasedMoved { movement, .. }
        | EventOutcome::LibraryTopMoved { movement, .. } => Some(movement),
        EventOutcome::StackObjectCountered { movement, .. } => movement.as_ref(),
        _ => None,
    }
}
impl RulesTxn {
    pub(crate) fn stack_limit(&self) -> usize {
        capacities::STACK_LIMIT
    }
    // CR 603.2c/603.10/701.26a: observe the source after a committed tap.
    // Entry tapped, an attempted tap of a tapped object, and mana reversal
    // have no tap occurrence. The player who caused the tap is irrelevant.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    fn source_tap_witness(&self, object: ObjectId) -> Result<Option<TriggerEventWitness>, String> {
        let current = self.current(object)?;
        Ok((current.present_on_battlefield()
            && current.tapped
            && self
                .copiable_definition(object)?
                .triggered
                .iter()
                .any(|ability| {
                    matches!(
                        ability.condition.pattern(),
                        EventPattern::BecomesTapped { .. }
                    )
                }))
        .then_some(TriggerEventWitness::Tapped {
            object,
            controller: current.controller,
        }))
    }

    // CR 702.37e, 708.8: observe the source after it is turned face up,
    // with the abilities it now has. Entry abilities do not trigger.
    fn source_face_up_witness(
        &self,
        object: ObjectId,
    ) -> Result<Option<TriggerEventWitness>, String> {
        let current = self.current(object)?;
        Ok((current.present_on_battlefield()
            && !current.face_down
            && self
                .copiable_definition(object)?
                .triggered
                .iter()
                .any(|ability| {
                    matches!(
                        ability.condition.pattern(),
                        EventPattern::TurnedFaceUp { .. }
                    )
                }))
        .then_some(TriggerEventWitness::TurnedFaceUp {
            object,
            controller: current.controller,
        }))
    }

    pub(super) fn record_turn_occurrences(
        &mut self,
        before: &RulesTxn,
        occurrences: &[OccurrenceResult],
    ) -> Result<(), String> {
        // CR 603.2c, 603.10a: every member of a simultaneous occurrence
        // contributes before any member is checked. Departures use LKI.
        for occurrence in occurrences {
            let departure = match &occurrence.actual {
                EventOutcome::LeftGame { object, .. } => Some((*object, false)),
                actual => movement(actual)
                    .filter(|m| m.from == Zone::Battlefield && m.destination != Zone::Battlefield)
                    .map(|m| (m.previous, matches!(m.destination, Zone::Graveyard { .. }))),
            };
            if let Some((object, to_graveyard)) = departure
                && before
                    .current(object)
                    .is_ok_and(|o| o.present_on_battlefield())
            {
                let prior = before.object_before(object)?;
                let row = &mut self.rules_mut().cast_history[prior.controller.0 as usize];
                row.permanent_left = true;
                row.creature_died |=
                    to_graveyard && prior.characteristics.has_type(CardKind::Creature);
            }
            match &occurrence.actual {
                EventOutcome::LifeGained { player, amount, .. } => {
                    self.rules_mut().cast_history[player.0 as usize]
                        .life_gained
                        .add(amount);
                }
                EventOutcome::DamageCommitted { life_gains, .. }
                | EventOutcome::NoncombatDamageDealt { life_gains, .. }
                | EventOutcome::DamageDealt { life_gains, .. } => {
                    for gain in life_gains {
                        self.rules_mut().cast_history[gain.player.0 as usize]
                            .life_gained
                            .add(&gain.amount);
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    // CR 603.2c/603.3a/603.10a: one match per actual occurrence, with
    // pre-event lookback for dies and post-event observers for enters. An
    // atomic simultaneous group shares one observer view, including sources
    // that die together. LeaveGame/exile/replaced command moves are not dies.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    pub(crate) fn collect_triggers(
        &mut self,
        cause: &BatchCause,
        before: &RulesTxn,
        occurrences: &[OccurrenceResult],
    ) -> Result<(), String> {
        for occurrence in occurrences {
            if !self.state.rules.resolution_triggers.is_empty() {
                self.collect_resolution_departure(occurrence, before)?;
            }
            let mut witnesses = match &occurrence.actual {
                // CR 120.4b/120.8, 510.2, 603.2c/g, 615.6: match actual
                // positive members against their frozen pre-commit transaction.
                // Aggregate life results are not damage occurrences. The combat
                // source/player uniqueness invariant is enforced at preparation,
                // restore validation and the legacy leaf before any writes.
                // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
                EventOutcome::DamageCommitted {
                    transaction,
                    actual,
                    ..
                } => {
                    let tx = before.damage_transaction()?;
                    if tx.id != *transaction {
                        return Err("damage trigger transaction identity invariant".into());
                    }
                    let mut ids = BTreeSet::new();
                    let mut witnesses = vec![];
                    for d in actual {
                        let entry = tx
                            .entries
                            .get(d.id.0 as usize)
                            .filter(|e| e.id == d.id)
                            .ok_or("damage trigger missing actual member")?;
                        if !ids.insert(d.id)
                            || d.amount <= Quantity::zero()
                            || entry.source != d.source
                            || entry.controller != d.controller
                            || entry.recipient != d.recipient
                            || entry.amount != d.amount
                        {
                            return Err("damage trigger actual member invariant".into());
                        }
                        if matches!(d.recipient, DamageRecipient::Player { .. }) {
                            witnesses.push(TriggerEventWitness::DamageDealt {
                                kind: tx.kind,
                                transaction: Some(tx.id),
                                damage: Box::new(d.clone()),
                                match_source: None,
                            });
                        }
                    }
                    witnesses
                }
                // The genuine legacy noncombat leaf carries the consumed
                // source facts and invocation controller separately. Its typed
                // occurrence, not its life-total result, supplies the witness.
                EventOutcome::NoncombatDamageDealt { damage, .. }
                    if matches!(damage.recipient, DamageRecipient::Player { .. }) =>
                {
                    vec![TriggerEventWitness::DamageDealt {
                        kind: DamageKind::Noncombat,
                        transaction: None,
                        match_source: None,
                        damage: Box::new(ActualDamage {
                            id: DamageId(0),
                            source: damage.source.clone(),
                            controller: damage.controller,
                            recipient: damage.recipient.clone(),
                            amount: damage.amount.clone(),
                        }),
                    }]
                }
                // A genuine legacy CombatDamage leaf has no transaction ID.
                // Its typed vector supplies source, recipient and amount; freeze
                // pre-event source facts, never infer from its life-total result.
                EventOutcome::DamageDealt { damage, .. } => damage
                    .iter()
                    .enumerate()
                    .filter(|(_, d)| matches!(d.recipient, DamageRecipient::Player { .. }))
                    .map(|(i, d)| {
                        Ok(TriggerEventWitness::DamageDealt {
                            kind: DamageKind::Combat,
                            transaction: None,
                            match_source: None,
                            damage: Box::new(ActualDamage {
                                id: DamageId(
                                    u32::try_from(i).map_err(|_| "damage member identity limit")?,
                                ),
                                source: before.object_before(d.source)?,
                                controller: d.controller,
                                recipient: d.recipient.clone(),
                                amount: d.amount.clone(),
                            }),
                        })
                    })
                    .collect::<Result<Vec<_>, String>>()?,
                // CR 508.1f-m/508.2a-b/508.3a/508.4, 603.2c/g: only
                // the completed, validated declaration is an attack event.
                // Each member shares the real batch occurrence but freezes its
                // own exact attacker/defender. Taps, combat flags and entry
                // attacking never fabricate occurrences. Capture pre-event
                // attacker facts, not mutable later combat membership.
                // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
                EventOutcome::AttackersDeclared { attacks } => {
                    let mut witnesses = Vec::new();
                    for (attacker, defender) in attacks {
                        let prior = before.object_before(*attacker)?;
                        // The declaration leaf applies its taps internally
                        // (508.1f). Vigilance leaves the source untapped.
                        if !prior.tapped
                            && let Some(tap) = self.source_tap_witness(*attacker)?
                        {
                            witnesses.push(tap);
                        }
                        witnesses.push(TriggerEventWitness::CreatureAttacked {
                            attacker: Box::new(prior),
                            binding: None,
                            defender_player_id: *defender,
                        });
                    }
                    witnesses
                }
                // CR 509.1h, 509.3g, 802.4: one declaration for each actual
                // defending player. A previously blocked attacker stays blocked.
                EventOutcome::BlockersDeclared { player, .. } => self
                    .state
                    .rules
                    .combat
                    .iter()
                    .filter(|attack| attack.defender == *player && !attack.blocked)
                    .map(|attack| {
                        let attacker = self.object_before(attack.attacker)?;
                        Ok(TriggerEventWitness::AttackerUnblocked {
                            defender: *player,
                            controller_at_match: attacker.controller,
                            attacker: Box::new(attacker),
                        })
                    })
                    .collect::<Result<Vec<_>, String>>()?,
                EventOutcome::Tapped { object }
                | EventOutcome::Regenerated {
                    object,
                    tapped: true,
                    ..
                } => self.source_tap_witness(*object)?.into_iter().collect(),
                EventOutcome::TurnedFaceUp { object, .. } => {
                    self.source_face_up_witness(*object)?.into_iter().collect()
                }
                EventOutcome::TokenCopied { object, .. } => {
                    vec![TriggerEventWitness::TokenEntered { object: *object }]
                }
                EventOutcome::TokensCreated { objects, .. }
                | EventOutcome::PredefinedTokensCreated { objects, .. } => objects
                    .iter()
                    .map(|object| TriggerEventWitness::TokenEntered { object: *object })
                    .collect(),
                // CR 603.2b/503.1a/504.1/117.5: collect at the actual
                // boundary; EnterStep still owns the normal draw/replacement.
                // Stabilization/APNAP placement follows those entry actions.
                EventOutcome::TurnTransitioned { current, owner, .. } => {
                    vec![TriggerEventWitness::StepBegan {
                        turn: current.clone(),
                        owner: *owner,
                        active: self.state.rules.active,
                    }]
                }

                // CR 121.1–2, 121.5, 903.9b: only a completed draw's
                // library-to-hand move counts. Replacement moves and an
                // unsuccessful attempt do not trigger.
                EventOutcome::Moved { movement }
                    if matches!(cause, BatchCause::DrawStep { player }
                        | BatchCause::InstructionDraw { player, .. }
                        if movement.from == (Zone::Library { owner: *player })
                            && movement.destination == (Zone::Hand { owner: *player })) =>
                {
                    let Zone::Hand { owner: player } = movement.destination else {
                        return Err("draw witness has no hand destination".into());
                    };
                    vec![TriggerEventWitness::CardDrawn {
                        player,
                        controller_at_match: player,
                        nth: None,
                    }]
                }
                EventOutcome::Milled { movements, .. }
                | EventOutcome::Surveilled { movements, .. } => movements
                    .iter()
                    .map(|movement| TriggerEventWitness::Moved {
                        movement: movement.clone(),
                    })
                    .collect(),
                actual => movement(actual)
                    .map(|m| {
                        vec![TriggerEventWitness::Moved {
                            movement: m.clone(),
                        }]
                    })
                    .unwrap_or_default(),
            };
            // CR 119.9, 120.3f: each source's positive life gain is its own
            // life-gain event; lifelink gains arrive grouped by source.
            let gains: Vec<(PlayerId, &Quantity)> = match &occurrence.actual {
                EventOutcome::LifeGained { player, amount, .. } => vec![(*player, amount)],
                EventOutcome::DamageCommitted { life_gains, .. }
                | EventOutcome::NoncombatDamageDealt { life_gains, .. }
                | EventOutcome::DamageDealt { life_gains, .. } => life_gains
                    .iter()
                    .map(|gain| (gain.player, &gain.amount))
                    .collect(),
                _ => vec![],
            };
            witnesses.extend(
                gains
                    .into_iter()
                    .filter(|(_, amount)| **amount > Quantity::zero())
                    .map(|(player, amount)| TriggerEventWitness::LifeGained {
                        player,
                        amount: amount.clone(),
                        controller_at_match: player,
                        nth: None,
                    }),
            );
            // CR 509.3a, 509.3c: each declared blocker once, and each attacker
            // that was unblocked until this declaration, as they were before.
            if let EventOutcome::BlockersDeclared { blocks, .. } = &occurrence.actual {
                let (mut blockers, mut attackers) = (BTreeSet::new(), BTreeSet::new());
                for (blocker, attacker) in blocks {
                    if blockers.insert(*blocker) {
                        let facts = before.object_before(*blocker)?;
                        witnesses.push(TriggerEventWitness::Blocked {
                            controller_at_match: facts.controller,
                            blocker: Box::new(facts),
                        });
                    }
                    if attackers.insert(*attacker)
                        && before
                            .state
                            .rules
                            .combat
                            .iter()
                            .any(|a| a.attacker == *attacker && !a.blocked)
                    {
                        let facts = before.object_before(*attacker)?;
                        witnesses.push(TriggerEventWitness::BecameBlocked {
                            controller_at_match: facts.controller,
                            attacker: Box::new(facts),
                        });
                    }
                }
            }
            // CR 120.3, 603.2c: each permanent dealt positive damage by this
            // simultaneous event, once however many sources dealt it.
            let damaged: Vec<(DamageKind, ObjectId)> = match &occurrence.actual {
                EventOutcome::DamageCommitted { actual, .. } => {
                    let kind = before.damage_transaction()?.kind;
                    actual
                        .iter()
                        .filter(|d| d.amount > Quantity::zero())
                        .filter_map(|d| match d.recipient {
                            DamageRecipient::Creature { object } => Some((kind, object)),
                            DamageRecipient::Player { .. } => None,
                        })
                        .collect()
                }
                EventOutcome::NoncombatDamageDealt { damage, .. } => match damage.recipient {
                    DamageRecipient::Creature { object } if damage.amount > Quantity::zero() => {
                        vec![(DamageKind::Noncombat, object)]
                    }
                    _ => vec![],
                },
                EventOutcome::DamageDealt { damage, .. } => damage
                    .iter()
                    .filter(|d| d.amount > Quantity::zero())
                    .filter_map(|d| match d.recipient {
                        DamageRecipient::Creature { object } => Some((DamageKind::Combat, object)),
                        DamageRecipient::Player { .. } => None,
                    })
                    .collect(),
                _ => vec![],
            };
            let mut recipients = BTreeSet::new();
            for (kind, object) in damaged {
                if recipients.insert(object)
                    && before
                        .current(object)
                        .is_ok_and(|o| o.present_on_battlefield())
                {
                    let facts = before.object_before(object)?;
                    witnesses.push(TriggerEventWitness::PermanentDamaged {
                        kind,
                        controller_at_match: facts.controller,
                        recipient: Box::new(facts),
                    });
                }
            }
            // CR 603.6c, 701.9a, 701.21a: departures and the discard or
            // sacrifice itself, beside any zone change.
            match &occurrence.actual {
                EventOutcome::LeftGame { object, owner }
                    if before
                        .current(*object)
                        .is_ok_and(|o| o.present_on_battlefield()) =>
                {
                    witnesses.push(TriggerEventWitness::PermanentLeftGame {
                        object: *object,
                        owner: *owner,
                    });
                }
                EventOutcome::Sacrificed { player, movement } => {
                    witnesses.push(TriggerEventWitness::Sacrificed {
                        player: *player,
                        object: Box::new(before.object_before(movement.previous)?),
                        controller_at_match: *player,
                    });
                }
                EventOutcome::CardDiscarded { player, movement } => {
                    witnesses.push(TriggerEventWitness::Discarded {
                        player: *player,
                        movement: movement.clone(),
                        controller_at_match: *player,
                    });
                }
                _ => {}
            }
            for witness in witnesses {
                self.collect_resolution_step(&occurrence.id, &witness)?;
                if witness.dies()
                    || witness.enters()
                    || matches!(
                        witness,
                        TriggerEventWitness::StepBegan { .. }
                            | TriggerEventWitness::CreatureAttacked { .. }
                            | TriggerEventWitness::CardDrawn { .. }
                            | TriggerEventWitness::AttackerUnblocked { .. }
                            | TriggerEventWitness::DamageDealt { .. }
                            | TriggerEventWitness::Tapped { .. }
                            | TriggerEventWitness::TurnedFaceUp { .. }
                            | TriggerEventWitness::LifeGained { .. }
                            | TriggerEventWitness::Discarded { .. }
                            | TriggerEventWitness::Sacrificed { .. }
                            | TriggerEventWitness::Blocked { .. }
                            | TriggerEventWitness::BecameBlocked { .. }
                            | TriggerEventWitness::PermanentDamaged { .. }
                    )
                    || witness.leaves()
                {
                    self.collect_trigger_witness(
                        Some((before, cause)),
                        TriggerOccurrenceId::Batch(occurrence.id.clone()),
                        witness,
                    )?;
                }
            }
        }
        Ok(())
    }
    // CR 601.2i/603.2g/603.3: only the complete successful casting path calls
    // this, after every cost and the real SpellCast event, before priority/SBAs.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    pub(crate) fn collect_cast_triggers(&mut self, entry: &StackInstance) -> Result<(), String> {
        let completed = self.top()?;
        if completed.id != entry.id || !completed.kind.is_spell() || completed.paid.is_none() {
            return Err("cast occurrence requires a completed paid spell".into());
        }
        let prior = self
            .state
            .rules
            .cast_history
            .get(entry.controller.0 as usize)
            .ok_or("invalid cast history seat")?;
        let witness = TriggerEventWitness::SpellCast {
            cast: Box::new(CastOccurrence {
                history: CastHistoryFacts {
                    turn_number: self.state.rules.turn.number,
                    first_overall: !prior.any_cast,
                    first_from_exile: entry.cast_from == Zone::Exile && !prior.exile_cast,
                },
                x: entry.x.clone(),
                stack_id: entry.id,
                caster: entry.controller,
                cast_from: entry.cast_from,
                priority_paid_exile: entry.cast_from == Zone::Exile,
                spell: self.object_before(entry.source)?,
                nth: None,
                targets: Self::targeted_permanents(entry).into_iter().collect(),
            }),
        };
        self.collect_trigger_witness(
            None,
            TriggerOccurrenceId::Cast(CastOccurrenceId { cast: entry.id }),
            witness,
        )
    }
    pub(super) fn collect_trigger_witness(
        &mut self,
        before: Option<(&RulesTxn, &BatchCause)>,
        event: TriggerOccurrenceId,
        witness: TriggerEventWitness,
    ) -> Result<(), String> {
        // Prepare the complete payload/history transaction off-state as well.
        // Even a direct collector failure publishes no sibling, handle or trace.
        let mut staged = self.fork();
        staged.in_rule_step = self.in_rule_step;
        match staged.collect_trigger_witness_staged(before, event, witness) {
            Ok(()) => {
                self.commit_staged_state(staged);
                Ok(())
            }
            Err(reason) => {
                self.rules_mut().typed_fault = staged.state.rules.typed_fault.clone();
                Err(reason)
            }
        }
    }
    fn collect_trigger_witness_staged(
        &mut self,
        before: Option<(&RulesTxn, &BatchCause)>,
        event: TriggerOccurrenceId,
        witness: TriggerEventWitness,
    ) -> Result<(), String> {
        // Inside the full collector transaction, including the zero-match case.
        // Never consume on proposal, failed payment or a failed sibling batch.
        if let TriggerEventWitness::SpellCast { cast } = &witness {
            // CR 202.3e, 608.2i: measure mana value on the stack with paid X,
            // independently of costs and the card's later zone.
            let mana_value = self.stack_mana_value(self.top()?)?;
            let row = self
                .rules_mut()
                .cast_history
                .get_mut(cast.caster.0 as usize)
                .ok_or("invalid cast history seat")?;
            row.any_cast = true;
            row.exile_cast |= cast.cast_from == Zone::Exile;
            row.mana_value.add(&mana_value);
            row.spells.push(cast.spell.clone());
            if cast.cast_from == (Zone::Hand { owner: cast.caster }) {
                row.hand_casts = row
                    .hand_casts
                    .checked_add(1)
                    .ok_or("turn history count limit")?;
            }
        }
        // CR 121.2, 119.9: count the turn's draws and life-gain events.
        if let TriggerEventWitness::CardDrawn { player, .. }
        | TriggerEventWitness::LifeGained { player, .. } = &witness
        {
            let draw = matches!(witness, TriggerEventWitness::CardDrawn { .. });
            let row = self
                .rules_mut()
                .cast_history
                .get_mut(player.0 as usize)
                .ok_or("invalid turn history seat")?;
            let count = if draw {
                &mut row.draws
            } else {
                &mut row.life_gains
            };
            *count = count.checked_add(1).ok_or("turn history count limit")?;
        }
        let dies = witness.dies();
        let enters = witness.enters();

        let subject = witness.subject();
        let observers = if witness.looks_back() {
            before
                .ok_or("leave trigger requires actual pre-event lookback")?
                .0
        } else {
            &*self
        };
        // Capture from the actual event's resulting game, including genuine
        // legacy leaves. Prepared damage retains assignment-time facts, while
        // a damage source's predicate reads this exact incarnation still on
        // the battlefield under the continuous effects that exist now (603.10).
        let damage_match_source = match &witness {
            TriggerEventWitness::DamageDealt { damage, .. }
                if observers
                    .current(damage.source.object)
                    .is_ok_and(|o| o.present_on_battlefield()) =>
            {
                Some(Box::new(observers.object_before(damage.source.object)?))
            }
            _ => None,
        };
        // CR 700.4, 603.10a: a dying object is read on the battlefield
        // immediately before the event.
        let dying = if dies || witness.leaves() {
            let subject = subject.ok_or("missing object trigger subject")?;
            observers
                .current(subject)
                .ok()
                .filter(|o| o.present_on_battlefield())
        } else {
            None
        };
        let token = |object: ObjectId| observers.state.rules.tokens.contains_key(&object.card);
        let mut matched = vec![];
        // Filled once per actual entry, only for observers that need its characteristics.
        let mut entry_facts = None;
        for object in observers
            .state
            .rules
            .objects
            .values()
            .filter(|o| o.present_on_battlefield())
            .filter(|o| !matches!(&witness, TriggerEventWitness::Tapped { object, .. }
                | TriggerEventWitness::TurnedFaceUp { object, .. } if o.id != *object))
        {
            let you = object.controller;
            let identity = |r: &Ref| match r {
                Ref::Source => Some(object.id),
                Ref::AttachedTo => object.attached_to,
                _ => None,
            };
            for (ability, definition) in observers
                .copiable_definition(object.id)?
                .triggered
                .iter()
                .enumerate()
            {
                let mut entry_witness = None;
                let Some(pattern) = witness.branch(definition.condition.pattern()) else {
                    continue;
                };
                let matches = match pattern {
                    EventPattern::Enters { object: pred } => enters && {
                        let subject = subject.ok_or("missing entrant identity")?;
                        if pred.reads_characteristics() || binds_observed_object(definition) {
                            if entry_facts.is_none() {
                                entry_facts = Some(observers.object_before(subject)?);
                            }
                            let entrant = entry_facts.as_ref().ok_or("missing entrant facts")?;
                            let facts = Frozen {
                                facts: entrant,
                                token: token(subject),
                                live: observers.is_live(entrant.controller),
                            };
                            let found = entrant.zone == Zone::Battlefield
                                && holds(pred, Some(&facts), you, subject, &identity);
                            if found {
                                entry_witness = Some(self::entry_witness(entrant, &witness)?);
                            }
                            found
                        } else {
                            holds(pred, None::<&Frozen>, you, subject, &identity)
                        }
                    },
                    EventPattern::Dies { object: pred } => dies && dying.is_some_and(|dying| {
                        holds(pred, Some(&observers.object_facts(dying)), you, dying.id, &identity)
                    }),
                    EventPattern::BecomesTapped { object: pred } => {
                        matches!(&witness, TriggerEventWitness::Tapped { object: tapped, .. }
                            if observers.current(*tapped).is_ok_and(|t| {
                                holds(pred, Some(&observers.object_facts(t)), you, *tapped, &identity)
                            }))
                    }
                    EventPattern::TurnedFaceUp { object: pred } => {
                        matches!(&witness, TriggerEventWitness::TurnedFaceUp { object: turned, .. }
                            if observers.current(*turned).is_ok_and(|t| {
                                holds(pred, Some(&observers.object_facts(t)), you, *turned, &identity)
                            }))
                    }
                    EventPattern::Attacks { attacker: pred } => {
                        matches!(&witness, TriggerEventWitness::CreatureAttacked { attacker, .. }
                            if holds(pred, Some(&Frozen {
                                facts: attacker,
                                token: token(attacker.object),
                                live: observers.is_live(attacker.controller),
                            }), you, attacker.object, &identity))
                    }
                    EventPattern::Cast { caster, spell, targets_source, .. } => {
                        matches!(&witness, TriggerEventWitness::SpellCast { cast }
                            if player_in(caster, Some(cast.caster), you)
                                && (!*targets_source || cast.targets.contains(&object.id))
                                && holds(spell, Some(&Frozen {
                                    facts: &cast.spell,
                                    token: false,
                                    live: observers.is_live(cast.caster),
                                }), you, cast.spell.object, &identity))
                    }
                    EventPattern::AttacksUnblocked { attacker: pred } => {
                        matches!(&witness, TriggerEventWitness::AttackerUnblocked { attacker, .. }
                            if holds(pred, Some(&Frozen {
                                facts: attacker,
                                token: token(attacker.object),
                                live: observers.is_live(attacker.controller),
                            }), you, attacker.object, &identity))
                    }
                    EventPattern::Draws { player, .. } => {
                        matches!(&witness, TriggerEventWitness::CardDrawn { player: drawing, .. }
                            if player_in(player, Some(*drawing), you))
                    }
                    EventPattern::StepBegins { step, active: player } => {
                        matches!(&witness, TriggerEventWitness::StepBegan { turn, active, .. }
                            if step_matches(*step, turn) && player_in(player, *active, you))
                    }
                    EventPattern::DealsDamage {
                        source: pred,
                        combat,
                        recipient,
                    } => {
                        matches!(&witness, TriggerEventWitness::DamageDealt { kind, damage, .. }
                            if damage.amount > Quantity::zero()
                                && (!*combat || *kind == DamageKind::Combat)
                                && matches!(damage.recipient, DamageRecipient::Player { player }
                                    if player_in(recipient, Some(player), you))
                                // CR 609.7, 119.3: an identity-only source
                                // ("a source other than ...") needs no
                                // battlefield appearance; a spell, or an
                                // ability whose source has left, still
                                // deals the damage.
                                && if pred.reads_characteristics() {
                                    damage_match_source.as_deref().is_some_and(|appearance| {
                                        appearance.object == damage.source.object
                                            && appearance.zone == Zone::Battlefield
                                            && holds(pred, Some(&Frozen {
                                                facts: appearance,
                                                token: token(appearance.object),
                                                live: observers.is_live(appearance.controller),
                                            }), you, appearance.object, &identity)
                                    })
                                } else {
                                    identity_holds(pred, damage.source.object, &identity)
                                })
                    }
                    EventPattern::GainsLife { player, .. } => {
                        matches!(&witness, TriggerEventWitness::LifeGained { player: gaining, .. }
                            if player_in(player, Some(*gaining), you))
                    }
                    EventPattern::Discards { player } => {
                        matches!(&witness, TriggerEventWitness::Discarded { player: discarding, .. }
                            if player_in(player, Some(*discarding), you))
                    }
                    // CR 603.10a: the sacrificed permanent as it last
                    // existed on the battlefield.
                    EventPattern::Sacrifices { player, object: pred } => {
                        matches!(&witness, TriggerEventWitness::Sacrificed { player: actor, object: sacrificed, .. }
                            if player_in(player, Some(*actor), you)
                                && sacrificed.zone == Zone::Battlefield
                                && holds(pred, Some(&Frozen {
                                    facts: sacrificed,
                                    token: token(sacrificed.object),
                                    live: observers.is_live(sacrificed.controller),
                                }), you, sacrificed.object, &identity))
                    }
                    EventPattern::LeavesBattlefield { object: pred } => {
                        witness.leaves()
                            && dying.is_some_and(|leaving| {
                                holds(pred, Some(&observers.object_facts(leaving)), you, leaving.id, &identity)
                            })
                    }
                    EventPattern::Blocks { blocker: pred } => {
                        matches!(&witness, TriggerEventWitness::Blocked { blocker: facts, .. }
                            if holds(pred, Some(&Frozen {
                                facts,
                                token: token(facts.object),
                                live: observers.is_live(facts.controller),
                            }), you, facts.object, &identity))
                    }
                    EventPattern::BecomesBlocked { attacker: pred } => {
                        matches!(&witness, TriggerEventWitness::BecameBlocked { attacker: facts, .. }
                            if holds(pred, Some(&Frozen {
                                facts,
                                token: token(facts.object),
                                live: observers.is_live(facts.controller),
                            }), you, facts.object, &identity))
                    }
                    EventPattern::IsDealtDamage { object: pred, combat } => {
                        matches!(&witness, TriggerEventWitness::PermanentDamaged { kind, recipient: facts, .. }
                            if (!*combat || *kind == DamageKind::Combat)
                                && holds(pred, Some(&Frozen {
                                    facts,
                                    token: token(facts.object),
                                    live: observers.is_live(facts.controller),
                                }), you, facts.object, &identity))
                    }
                    EventPattern::BecomesTarget { .. } | EventPattern::AnyOf { .. } => false,
                };
                // CR 603.2: an ordinal event triggers only as the turn's Nth
                // matching event, counted from the ledger it already joined.
                let nth = match pattern.nth() {
                    Some(ordinal) if matches => self
                        .turn_count(pattern, &witness, you)?
                        .filter(|count| *count == ordinal.nth),
                    _ => None,
                };
                // "Triggers only once each turn": this incarnation's use, read
                // from the live ledger so simultaneous occurrences see it.
                let use_key = definition.condition.once_each_turn().then(|| TriggerUse {
                    source: object.id,
                    face: object.face(),
                    ability: ability as u32,
                });
                if matches
                    && (pattern.nth().is_none() || nth.is_some())
                    && use_key.is_none_or(|key| !self.state.rules.trigger_uses.contains(&key))
                    && matches!((definition.functioning_zone, object.zone), (FunctioningZone::Battlefield, Zone::Battlefield) | (FunctioningZone::Stack, Zone::Stack))
                    && observers.is_live(object.controller)
                {
                    let mut matched_witness = entry_witness.unwrap_or_else(|| witness.clone());
                    if let TriggerEventWitness::CardDrawn { controller_at_match, .. }
                        | TriggerEventWitness::AttackerUnblocked { controller_at_match, .. }
                        | TriggerEventWitness::LifeGained { controller_at_match, .. }
                        | TriggerEventWitness::Discarded { controller_at_match, .. }
                        | TriggerEventWitness::Sacrificed { controller_at_match, .. }
                        | TriggerEventWitness::Blocked { controller_at_match, .. }
                        | TriggerEventWitness::BecameBlocked { controller_at_match, .. }
                        | TriggerEventWitness::PermanentDamaged { controller_at_match, .. } = &mut matched_witness
                    {
                        *controller_at_match = you;
                    }
                    match &mut matched_witness {
                        TriggerEventWitness::CardDrawn { nth: slot, .. }
                        | TriggerEventWitness::LifeGained { nth: slot, .. } => *slot = nth,
                        TriggerEventWitness::SpellCast { cast } => cast.nth = nth,
                        _ => {}
                    }
                    if let TriggerEventWitness::DamageDealt { match_source, .. } =
                        &mut matched_witness
                    {
                        // The appearance is evidence only when the pattern
                        // read the source's characteristics.
                        *match_source = if pattern.object().is_some_and(Pred::reads_characteristics) {
                            damage_match_source.clone()
                        } else {
                            None
                        };
                    }
                    let observed = if binds_observed_object(definition) {
                        let object = subject.ok_or("missing bound event object")?;
                        Some(observers.object_before(object)?)
                    } else {
                        None
                    };
                    let intervening = choices::intervening(definition)?;
                    matched.push((
                        if binds_death_controller(definition) {
                            Some(
                                observers
                                    .current(subject.ok_or("missing object trigger subject")?)?
                                    .controller,
                            )
                        } else {
                            None
                        },
                        observers.object_before(object.id)?,
                        EffectiveTriggerAbility::Intrinsic { index: u32::try_from(ability).map_err(|_| "trigger ability index limit")? },
                        intervening
                            .as_ref()
                            .map(|condition| {
                                observers.check_intervening(
                                    condition,
                                    &self.state.rules.cast_history,
                                    object.controller,
                                    object.id,
                                    &observers.object_before(object.id)?,
                                    if *condition == kir::Intervening::SourceCastByController {
                                        before.and_then(|(prior, cause)| prior.completed_entry_cast(cause, &witness))
                                    } else {
                                        None
                                    },
                                )
                            })
                            .transpose()?,
                        matched_witness,
                        observed,
                        use_key,
                ));
                }
            }
        }

        // Preflight the COMPLETE effective emission before diagnostics, handles,
        // IDs or pending rows publish. Failed intervening checks still retain
        // their ordinary diagnostic below, but consume no trigger capacity.
        let emission = matched
            .iter()
            .filter(|(_, _, _, condition, _, _, _)| condition.as_ref().is_none_or(|c| c.passed()))
            .count();
        self.preflight_trigger_emission(emission)?;
        for (
            death_controller,
            source_facts,
            ability,
            match_condition,
            mut witness,
            observed,
            use_key,
        ) in matched
        {
            if let Some(check) = &match_condition {
                self.record_event(Event::TriggerConditionChecked {
                    source: source_facts.object,
                    controller: source_facts.controller,
                    ability,
                    stage: InterveningStage::Match {
                        occurrence: event.clone(),
                    },
                    check: check.clone(),
                })?;
                if !check.passed() {
                    continue;
                }
            }
            // CR 603.4: only an ability that actually triggered uses its limit.
            if let Some(key) = use_key
                && !self.rules_mut().trigger_uses.insert(key)
            {
                return Err("trigger limit already used".into());
            }
            if let Some(facts) = observed {
                let binding = match &mut witness {
                    TriggerEventWitness::Entered { binding, .. }
                    | TriggerEventWitness::CreatureAttacked { binding, .. } => binding,
                    _ => return Err("bound event object has no typed witness".into()),
                };
                *binding = Some(ObservedObject {
                    controller_at_match: source_facts.controller,
                    handles: self.capture_trigger_source_handles(facts.object)?,
                    last_known: Box::new(facts),
                });
            }
            self.enqueue_trigger(
                ability,
                source_facts.controller,
                source_facts,
                death_controller,
                match_condition,
                (event.clone(), witness),
            )?;
        }
        self.check_history_capacity()?;
        Ok(())
    }
    /// The once-each-turn key a pending intrinsic trigger would have
    /// recorded: its source incarnation, the face up as it triggered and its
    /// ability index. Only a limited ability ever records it.
    pub(super) fn trigger_use(trigger: &PendingTrigger) -> Option<TriggerUse> {
        match trigger.ability {
            EffectiveTriggerAbility::Intrinsic { index } => Some(TriggerUse {
                source: trigger.source,
                face: trigger.source_face,
                ability: index,
            }),
            EffectiveTriggerAbility::Followup { .. } => None,
        }
    }
    /// The turn ledger's spells are this turn's frozen casts by their row's
    /// seat; a limited trigger's use names a card of this game. Replay
    /// proves the counts and uses themselves.
    pub(super) fn validate_turn_history(&self) -> Result<(), String> {
        for (seat, row) in self.state.rules.cast_history.iter().enumerate() {
            if row.attacked && seat != self.state.rules.turn_owner.0 as usize
                || row.hand_casts as usize > row.spells.len()
                || row.creature_died && !row.permanent_left
                || row.life_gained < Quantity::from(i64::from(row.life_gains))
                || (row.life_gained == Quantity::zero()) != (row.life_gains == 0)
            {
                return Err("snapshot turn history invariant".into());
            }
            for spell in &row.spells {
                self.validate_fact_identity(spell)?;
                if spell.controller.0 as usize != seat
                    || spell.zone != Zone::Stack
                    || self
                        .state
                        .rules
                        .cards
                        .get(&spell.object.card)
                        .is_none_or(|c| c.owner != spell.owner)
                {
                    return Err("snapshot turn cast history invariant".into());
                }
            }
        }
        if self.state.rules.trigger_uses.iter().any(|key| {
            self.effective_trigger(
                key.source,
                key.face,
                EffectiveTriggerAbility::Intrinsic { index: key.ability },
            )
            .map_or(true, |ability| !ability.condition.once_each_turn())
        }) {
            return Err("snapshot trigger use invariant".into());
        }
        Ok(())
    }
    /// CR 603.2: the event's count among the current turn's events that
    /// match an ordinal pattern, from the turn ledger the event has joined.
    /// "Your/their Nth" counts the event player's events; `any_player`
    /// counts every player's spells. A filtered spell ordinal counts only
    /// spells that matched the filter as they were cast.
    fn turn_count(
        &self,
        pattern: &EventPattern,
        witness: &TriggerEventWitness,
        you: PlayerId,
    ) -> Result<Option<u32>, String> {
        let rows = &self.state.rules.cast_history;
        let row = |player: PlayerId| {
            rows.get(player.0 as usize)
                .ok_or_else(|| "invalid turn history seat".to_string())
        };
        Ok(match (pattern, witness) {
            (
                EventPattern::Cast {
                    spell,
                    nth: Some(ordinal),
                    ..
                },
                TriggerEventWitness::SpellCast { cast },
            ) => {
                let seats: Vec<&TurnCastHistory> = if ordinal.any_player {
                    rows.iter().collect()
                } else {
                    vec![row(cast.caster)?]
                };
                let count = seats
                    .into_iter()
                    .flat_map(|r| &r.spells)
                    .filter(|facts| {
                        holds(
                            spell,
                            Some(&Frozen {
                                facts,
                                token: false,
                                live: self.is_live(facts.controller),
                            }),
                            you,
                            facts.object,
                            &|_: &Ref| None,
                        )
                    })
                    .count();
                Some(u32::try_from(count).map_err(|_| "turn history count limit")?)
            }
            (EventPattern::Draws { .. }, TriggerEventWitness::CardDrawn { player, .. }) => {
                Some(row(*player)?.draws)
            }
            (EventPattern::GainsLife { .. }, TriggerEventWitness::LifeGained { player, .. }) => {
                Some(row(*player)?.life_gains)
            }
            _ => None,
        })
    }
    pub(super) fn enqueue_trigger(
        &mut self,
        ability: EffectiveTriggerAbility,
        controller: PlayerId,
        source_facts: ObjectBefore,
        death_controller: Option<PlayerId>,
        match_condition: Option<InterveningCheck>,
        (occurrence, witness): (TriggerOccurrenceId, TriggerEventWitness),
    ) -> Result<TriggerId, String> {
        self.preflight_trigger_emission(1)?;
        let id = TriggerId(self.state.rules.next_trigger_id);
        self.rules_mut().next_trigger_id += 1;
        let source = source_facts.object;
        let source_handles = self.capture_trigger_source_handles(source)?;
        // CR 113.7a: the face the facts were captured with as it triggered.
        let source_face = face_after(source_facts.object.face, source_facts.transforms);
        self.rules_mut().pending_triggers.push(PendingTrigger {
            source_face,
            death_controller,
            match_condition,
            id,
            source,
            controller,
            ability,
            source_facts: source_facts.clone(),
            source_handles,
            event: occurrence.clone(),
            witness: witness.clone(),
        });
        self.record_event(Event::TriggerCollected {
            trigger_id: id,
            source,
            player: controller,
            ability,
            occurrence,
            witness,
            source_facts: Box::new(source_facts),
        })?;
        Ok(id)
    }

    pub(crate) fn preflight_trigger_emission(&mut self, additional: usize) -> Result<(), String> {
        self.check_stack_capacity(additional)?;
        let additional = u64::try_from(additional).map_err(|_| "trigger ID resource limit")?;
        if self
            .state
            .rules
            .next_trigger_id
            .checked_add(additional)
            .is_none()
        {
            return Err(self.capacity_error(RuleFault::TriggerIdCapacityExceeded {
                next: self.state.rules.next_trigger_id,
                additional,
            }));
        }
        self.preflight_stack_ids(additional)
    }
    pub(super) fn preflight_stack_ids(&mut self, additional: u64) -> Result<(), String> {
        // One stack ID per pending trigger, plus any new entries.
        let pending = u64::try_from(self.state.rules.pending_triggers.len())
            .map_err(|_| "stack ID resource limit")?;
        let requested = pending
            .checked_add(additional)
            .ok_or("stack ID resource limit")?;
        if u64::from(self.state.rules.next_stack_id)
            .checked_add(requested)
            .is_none_or(|end| end > u64::from(u32::MAX))
        {
            return Err(self.capacity_error(RuleFault::StackIdCapacityExceeded {
                next: self.state.rules.next_stack_id,
                additional: requested,
            }));
        }
        Ok(())
    }
    pub(super) fn apnap_seats(&self) -> impl Iterator<Item = PlayerId> {
        let anchor = self.state.rules.turn_owner.0;
        (0..4).map(move |offset| PlayerId((anchor + offset) % 4))
    }
    pub(super) fn apnap_players(&self) -> Vec<PlayerId> {
        self.apnap_seats().filter(|p| self.is_live(*p)).collect()
    }
    pub(crate) fn begin_trigger_placement(
        &mut self,
        resume: Frame,
        cleanup: bool,
        changed: bool,
    ) -> Result<bool, String> {
        if self.state.rules.pending_triggers.is_empty() {
            return Ok(false);
        }
        let live = self.live_players();
        self.rules_mut()
            .pending_triggers
            .retain(|t| live.contains(&t.controller));
        if self.state.rules.pending_triggers.is_empty() {
            return Ok(false);
        }
        self.rules_mut().frame = Frame::TriggerOrder {
            placement: TriggerPlacement {
                resume: self.save_frame(resume),
                cleanup,
                changed,
                remaining: self.apnap_players(),
                before: self.state.rules.next_trigger_id,
                order: vec![],
            },
        };
        Ok(true)
    }
    fn group_ids(&self, placement: &TriggerPlacement) -> Vec<TriggerId> {
        let Some(player) = placement.remaining.first() else {
            return vec![];
        };
        self.state
            .rules
            .pending_triggers
            .iter()
            .filter(|t| t.id.0 < placement.before && t.controller == *player)
            .map(|t| t.id)
            .collect()
    }
    pub(crate) fn trigger_order_ids(&self) -> Vec<TriggerId> {
        if let Frame::TriggerOrder { placement } = &self.state.rules.frame {
            if placement.order.is_empty() {
                self.group_ids(placement)
            } else {
                vec![]
            }
        } else {
            vec![]
        }
    }

    pub(super) fn trigger_request(&self) -> Result<Option<Decision<Rules>>, String> {
        let Frame::TriggerOrder { placement } = &self.state.rules.frame else {
            return Ok(None);
        };
        let ids = self.trigger_order_ids();
        if ids.len() < 2 {
            return Ok(None);
        }
        let player = *placement
            .remaining
            .first()
            .ok_or("missing trigger controller")?;
        let count = ids.len() as u32;
        Ok(Some(
            self.order(
                player,
                Reason::OrderTriggers,
                ids.into_iter()
                    .map(|trigger| Candidate::Trigger { trigger })
                    .collect(),
                count,
            ),
        ))
    }

    fn finish_trigger_placement(&mut self, placement: TriggerPlacement) -> Result<(), String> {
        self.rules_mut().frame = Frame::Stabilize {
            resume: placement.resume,
            cleanup: placement.cleanup,
            changed: placement.changed,
        };
        Ok(())
    }
    // CR 117.5/603.3b-d: finish all SBAs first; place each controller's
    // triggers in their chosen bottom-to-top order, active player first.
    // Targets are chosen as each invocation is placed, with no priority or
    // mana window between placements. Then check SBAs/pending triggers again.
    pub(super) fn trigger_step(&mut self, frame: Frame) -> Result<(), String> {
        match frame {
            Frame::TriggerOrder { mut placement } => {
                if !placement.order.is_empty() {
                    let id = placement.order.remove(0);
                    let index = self
                        .state
                        .rules
                        .pending_triggers
                        .iter()
                        .position(|t| t.id == id)
                        .ok_or("missing ordered trigger")?;
                    let trigger = self.rules_mut().pending_triggers.remove(index);
                    self.validate_match_condition(
                        trigger.source,
                        trigger.source_face,
                        trigger.ability,
                        trigger.controller,
                        trigger.match_condition.as_ref(),
                        &trigger.witness,
                    )?;
                    if !self.is_live(trigger.controller) {
                        self.rules_mut().frame = Frame::TriggerOrder { placement };
                        return Ok(());
                    }
                    let stack_id = StackId(self.state.rules.next_stack_id);
                    self.rules_mut().next_stack_id = self
                        .state
                        .rules
                        .next_stack_id
                        .checked_add(1)
                        .ok_or("stack ID resource limit")?;
                    let targeted = match trigger.ability {
                        EffectiveTriggerAbility::Intrinsic { .. } => {
                            let ability = self.effective_trigger(
                                trigger.source,
                                trigger.source_face,
                                trigger.ability,
                            )?;
                            !ability.program.targets.is_empty() || ability.modal.is_some()
                        }
                        EffectiveTriggerAbility::Followup { creator } => {
                            !self.resolution_trigger_program(creator)?.targets.is_empty()
                        }
                    };
                    let zero = ManaCost {
                        generic: Quantity::zero(),
                        colored: vec![],
                    };
                    // CR 701.27f: the transform count as an ability is put on
                    // the stack, or as a delayed trigger was created.
                    let source_transforms = match trigger.ability {
                        EffectiveTriggerAbility::Followup { creator } => {
                            self.resolution_trigger_transforms(creator)?
                        }
                        EffectiveTriggerAbility::Intrinsic { .. } => self
                            .current(trigger.source)
                            .map_or(trigger.source_facts.transforms, |o| o.transforms),
                    };
                    self.rules_mut().stack.push(StackInstance {
                        target_triggers: vec![],
                        mode_mask: None,
                        hybrid_colors: vec![],
                        cost_lock: None,
                        selected_additional_costs: None,

                        x: None,
                        death_controller: trigger.death_controller,
                        sacrificed: None,
                        sacrificed_set: vec![],
                        chosen_payment: vec![],
                        effect_source_handles: vec![],
                        source_facts: Some(trigger.source_facts.clone()),
                        entity_targets: vec![],
                        announced_entity_targets: vec![],
                        stack_targets: vec![],
                        announced_stack_targets: vec![],
                        source_face: trigger.source_face,
                        source_transforms,
                        kind: StackKind::Triggered {
                            match_condition: trigger.match_condition,
                            ability: trigger.ability,

                            source_handles: trigger.source_handles,
                            trigger_id: id,
                            event: trigger.event,
                            witness: trigger.witness,
                        },
                        player_targets: vec![],
                        id: stack_id,
                        source: trigger.source,
                        controller: trigger.controller,
                        targets: vec![],
                        announced_targets: vec![vec![]; 4],
                        paid: (!targeted).then_some(zero.clone()),
                        mana_uncounterable: false,
                        total_cost: zero,
                        cast_from: trigger.source_facts.zone,
                    });
                    placement.changed |= placement.cleanup;
                    self.rules_mut().passes.clear();
                    if targeted {
                        self.rules_mut().frame = Frame::TriggerTargets {
                            placement,
                            stack_id,
                        };
                    } else {
                        self.record_event(Event::TriggerPlaced {
                            trigger_id: id,
                            stack_id,
                            player: trigger.controller,
                        })?;
                        self.rules_mut().frame = Frame::TriggerOrder { placement };
                    }
                } else if self.trigger_request()?.is_some() {
                    let input = self
                        .rules_mut()
                        .pending
                        .take()
                        .ok_or("missing trigger ordering input")?;
                    let order = input.triggers()?;
                    self.record_event(Event::TriggersOrdered {
                        player: input.player,
                        order: order.clone(),
                    })?;
                    placement.order = order;
                    self.rules_mut().frame = Frame::TriggerOrder { placement };
                } else if placement.remaining.is_empty() {
                    self.finish_trigger_placement(placement)?;
                } else {
                    let candidates = self.group_ids(&placement);
                    if candidates.is_empty() {
                        placement.remaining.remove(0);
                    } else {
                        placement.order = candidates;
                    }
                    self.rules_mut().frame = Frame::TriggerOrder { placement };
                }
            }
            Frame::TriggerTargets {
                placement,
                stack_id,
            } => {
                if !self.top_has_target_domain()? {
                    self.rules_mut().pending = None;
                    return self.reject_trigger_placement(
                        placement,
                        stack_id,
                        TriggerPlacementFailure::NoLegalTargets,
                    );
                }
                if self.unbound_modes(self.top()?)? {
                    return self.bind_trigger_modes(placement, stack_id);
                }
                if self.program()?.targets.first().is_some_and(|t| {
                    matches!(
                        t.domain,
                        TargetDomain::BattlefieldPermanent
                            | TargetDomain::ControlledBattlefieldPermanent
                            | TargetDomain::BattlefieldCreature
                            | TargetDomain::ControlledBattlefieldCreature
                            | TargetDomain::OwnGraveyardCreatureCard
                            | TargetDomain::GraveyardCard { .. }
                            | TargetDomain::OwnGraveyardCard
                    )
                }) {
                    return self.bind_trigger_object_targets(placement, stack_id);
                }
                if self
                    .program()?
                    .targets
                    .first()
                    .is_some_and(|t| t.domain == TargetDomain::AnyDamageTarget)
                {
                    return self.bind_trigger_entity_targets(placement, stack_id);
                }
                let input = self
                    .rules_mut()
                    .pending
                    .take()
                    .ok_or("missing trigger target input")?;
                let players = input.players()?;
                let top = self
                    .rules_mut()
                    .stack
                    .last_mut()
                    .ok_or("missing targeted trigger")?;
                if top.id != stack_id {
                    return Err("targeted trigger identity invariant".into());
                }
                let StackKind::Triggered { trigger_id, .. } = top.kind else {
                    return Err("targeted trigger kind invariant".into());
                };
                top.player_targets = players;
                top.paid = Some(top.total_cost.clone());
                let controller = top.controller;
                self.record_event(Event::TriggerPlaced {
                    trigger_id,
                    stack_id,
                    player: controller,
                })?;
                self.rules_mut().frame = Frame::TriggerOrder { placement };
            }
            _ => return Err("trigger placement frame invariant".into()),
        }
        Ok(())
    }
    pub(super) fn revalidate_trigger_departure(&mut self) -> Result<(), String> {
        let live = self.live_players();
        self.rules_mut()
            .pending_triggers
            .retain(|t| live.contains(&t.controller));
        let revalidate = |mut placement: TriggerPlacement, game: &RulesTxn| {
            placement.remaining.retain(|p| game.is_live(*p));
            placement.order.retain(|id| {
                game.state
                    .rules
                    .pending_triggers
                    .iter()
                    .any(|t| t.id == *id)
            });
            placement
        };
        match &self.state.rules.frame {
            Frame::TriggerOrder { placement } => {
                self.rules_mut().frame = Frame::TriggerOrder {
                    placement: revalidate(placement.clone(), self),
                };
                if self
                    .state
                    .rules
                    .pending
                    .as_ref()
                    .is_some_and(|p| self.validate_pending(p).is_err())
                {
                    self.rules_mut().pending = None;
                }
            }
            Frame::TriggerTargets {
                placement,
                stack_id,
            } => {
                let stack_id = *stack_id;
                let placement = revalidate(placement.clone(), self);
                if !self.state.rules.stack.iter().any(|s| s.id == stack_id) {
                    self.rules_mut().pending = None;
                    self.rules_mut().frame = Frame::TriggerOrder { placement };
                } else {
                    self.rules_mut().frame = Frame::TriggerTargets {
                        placement,
                        stack_id,
                    };
                    if self
                        .state
                        .rules
                        .pending
                        .as_ref()
                        .is_some_and(|p| self.validate_pending(p).is_err())
                    {
                        self.rules_mut().pending = None;
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }
    fn validate_trigger_source(
        &self,
        source: ObjectId,
        face: CardFace,
        ability: EffectiveTriggerAbility,
        controller: PlayerId,
        fact: &ObjectBefore,
        handles: &[publish::StoredObjectHandle],
    ) -> Result<(), String> {
        let intrinsic = match ability {
            EffectiveTriggerAbility::Intrinsic { .. } => {
                Some(self.effective_trigger(source, face, ability)?)
            }
            EffectiveTriggerAbility::Followup { .. } => None,
        };
        // CR 712.18: only a transforming card's ability can come from a
        // face other than the one its incarnation entered with. A bound
        // entrant's witness preserves its controller at entry even if a
        // later departure refreshes the source facts under different control.
        if controller.0 >= 4
            || (face != source.face && !self.definition(source.card)?.transforms())
            || fact.object != source
            || fact.owner
                != self
                    .state
                    .rules
                    .cards
                    .get(&source.card)
                    .ok_or("snapshot unknown trigger source")?
                    .owner
            || fact.controller.0 >= 4
            || fact.default_controller.0 >= 4
            || intrinsic.is_some_and(|definition| {
                definition.functioning_zone != FunctioningZone::Battlefield
                    || fact.zone != Zone::Battlefield
                    || (*definition.condition.pattern() == EventPattern::self_enters()
                        && !binds_observed_object(&definition)
                        && fact.controller != controller)
            })
            || handles.len() != 4
        {
            return Err("snapshot trigger source invariant".into());
        }
        {
            self.validate_characteristic_identity(
                source,
                fact.transforms,
                fact.face_down,
                &fact.characteristics,
            )?;
        }
        Ok(())
    }
    fn valid_entry_subject(
        &self,
        entrant: &ObjectBefore,
        movement: Option<&MoveOutcome>,
    ) -> Result<bool, String> {
        self.validate_fact_identity(entrant)?;
        let token = self.state.rules.tokens.get(&entrant.object.card);
        let actual_entry = match movement {
            Some(m) => {
                token.is_none_or(|t| {
                    t.is_copy() && m.from == Zone::Stack && m.previous.incarnation == 0
                }) && m.destination == Zone::Battlefield
                    && match m.from {
                        Zone::Battlefield => false,
                        Zone::Stack | Zone::Exile => true,
                        Zone::Hand { owner }
                        | Zone::Library { owner }
                        | Zone::Command { owner }
                        | Zone::Graveyard { owner } => owner == entrant.owner,
                    }
                    && m.successor == entrant.object
                    && m.previous.card == m.successor.card
                    && m.previous.incarnation.checked_add(1) == Some(m.successor.incarnation)
            }
            None => {
                token.is_some_and(|t| !t.is_copy())
                    && entrant.object.incarnation == 0
                    && entrant.default_controller == entrant.owner
            }
        };
        // The historical controller is a seat of this game, though that
        // player may since have left it (CR 800.4a).
        let seated = |p: PlayerId| self.state.rules.original_seating.contains(&p);
        Ok(actual_entry
            && seated(entrant.controller)
            && seated(entrant.default_controller)
            && entrant.marked_damage == Quantity::zero()
            && !entrant.deathtouch_damage
            && !entrant.attack_ready
            && self
                .state
                .rules
                .cards
                .get(&entrant.object.card)
                .is_some_and(|c| c.owner == entrant.owner))
    }
    fn valid_cast_facts(&self, cast: &CastOccurrence) -> Result<bool, String> {
        self.validate_fact_identity(&cast.spell)?;
        Ok(cast.history.turn_number > 0
            && cast.history.turn_number <= self.state.rules.turn.number
            && (!cast.history.first_from_exile || cast.cast_from == Zone::Exile)
            && (!cast.history.first_overall
                || cast.cast_from != Zone::Exile
                || cast.history.first_from_exile)
            && cast.stack_id.0 < self.state.rules.next_stack_id
            && cast.caster.0 < 4
            && cast.spell.controller == cast.caster
            && cast.spell.default_controller.0 < 4
            && cast.spell.zone == Zone::Stack
            && !self
                .state
                .rules
                .tokens
                .contains_key(&cast.spell.object.card)
            && !cast.spell.characteristics.has_type(CardKind::Land)
            && self
                .state
                .rules
                .cards
                .get(&cast.spell.object.card)
                .is_some_and(|c| c.owner == cast.spell.owner)
            && (matches!(cast.cast_from, Zone::Hand { owner } | Zone::Command { owner } | Zone::Graveyard { owner } if owner == cast.spell.owner)
                || cast.cast_from == Zone::Exile)
            && (!cast.priority_paid_exile || cast.cast_from == Zone::Exile)
            && cast.x.as_ref().is_none_or(|x| !x.is_negative())
            && cast
                .targets
                .iter()
                .all(|t| self.state.rules.cards.contains_key(&t.card)))
    }

    fn validate_trigger_witness(
        &self,
        source: ObjectId,
        face: CardFace,
        ability: EffectiveTriggerAbility,
        controller: PlayerId,
        death_controller: Option<PlayerId>,
        witness: &TriggerEventWitness,
    ) -> Result<(), String> {
        let definition = self.effective_trigger(source, face, ability)?;
        let pattern = witness
            .branch(definition.condition.pattern())
            .ok_or("snapshot trigger witness has no matching alternative")?;
        // The accepted-input journal authenticates the historical attachment;
        // an observer can move to another creature before its ability resolves,
        // so a host is any object other than the source itself.
        let identity = |subject: ObjectId| {
            move |r: &Ref| match r {
                Ref::Source => Some(source),
                Ref::AttachedTo => (subject != source).then_some(subject),
                _ => None,
            }
        };
        let token = |object: ObjectId| self.state.rules.tokens.contains_key(&object.card);
        // CR 800.4a: the frozen facts are the object's at the event, when its
        // controller was in the game; a later departure does not undo them.
        let frozen = |facts| Frozen {
            facts,
            token: token(facts.object),
            live: true,
        };
        // CR 700.4: a pattern that names a creature reviewed the dying or
        // entering card as a creature card.
        let creature_card = |pred: &Pred, object: ObjectId| -> Result<bool, String> {
            Ok(!pred.0.contains(&Atom::Type {
                kind: CardKind::Creature,
            }) || self
                .historical_creature_possible(&self.copiable_definition(object)?.characteristics))
        };
        // CR 603.10a: exactly the reviewed opponent-death row freezes the
        // dying creature's controller, pending or on the stack alike.
        let binds_object = binds_observed_object(&definition);
        match witness.observed_object() {
            Some(binding) if binds_object => {
                let facts = &binding.last_known;
                if Some(facts.object) != witness.subject()
                    || binding.controller_at_match != controller
                    || facts.zone != Zone::Battlefield
                    || !self
                        .state
                        .rules
                        .original_seating
                        .contains(&facts.controller)
                    || !self
                        .state
                        .rules
                        .original_seating
                        .contains(&facts.default_controller)
                    || binding.handles.len() != 4
                    || self
                        .state
                        .rules
                        .cards
                        .get(&facts.object.card)
                        .is_none_or(|card| card.owner != facts.owner)
                {
                    return Err("snapshot event object binding invariant".into());
                }
                self.validate_fact_identity(facts)?;
            }
            None if !binds_object => {}
            _ => return Err("snapshot missing or unexpected event object binding".into()),
        }
        let binds = binds_death_controller(&definition);
        if death_controller.is_some() != binds
            || death_controller.is_some_and(|p| p.0 >= 4 || p == controller)
        {
            return Err("snapshot X/death LKI binding invariant".into());
        }
        let valid = match witness {
            TriggerEventWitness::ResolutionPaid { .. } => false,
            TriggerEventWitness::PermanentLeftGame { object, owner } => {
                self.state
                    .rules
                    .cards
                    .get(&object.card)
                    .is_some_and(|card| card.owner == *owner)
                    && !self.is_live(*owner)
                    && matches!(pattern, EventPattern::LeavesBattlefield { object: pred }
                        if identity_holds(pred, *object, &identity(*object)))
            }
            TriggerEventWitness::DamageDealt {
                kind,
                transaction,
                damage,
                match_source,
            } => {
                let fact = &damage.source;
                self.validate_fact_identity(fact)?;
                if let Some(appearance) = match_source {
                    self.validate_fact_identity(appearance)?;
                    if appearance.object != fact.object
                        || appearance.owner != fact.owner
                        || appearance.default_controller != fact.default_controller
                        || appearance.controller.0 >= 4
                        || appearance.default_controller.0 >= 4
                        || appearance.marked_damage.is_negative()
                    {
                        return Err("snapshot damage match appearance invariant".into());
                    }
                }
                transaction.is_none_or(|id| id.0 < self.state.rules.next_damage_transaction_id)
                    && (damage.id.0 as usize) < self.damage_limit()
                    && damage.amount > Quantity::zero()
                    // CR 120.2, 510.1: combat damage comes from battlefield
                    // creatures; noncombat damage may also come from a
                    // resolving instant or sorcery on the stack. A permanent
                    // spell deals no damage there (CR 608.3).
                    && (fact.zone == Zone::Battlefield
                        || (*kind != DamageKind::Combat
                            && fact.zone == Zone::Stack
                            && (fact.characteristics.has_type(CardKind::Instant)
                                || fact.characteristics.has_type(CardKind::Sorcery))))
                    // CR 510.1: only creatures deal combat damage.
                    && (*kind != DamageKind::Combat
                        || fact.characteristics.has_type(CardKind::Creature))
                    && (*kind != DamageKind::Combat || fact.controller == damage.controller)
                    && fact.controller.0 < 4
                    && damage.controller.0 < 4
                    && controller.0 < 4
                    && fact.default_controller.0 < 4
                    && self
                        .state
                        .rules
                        .cards
                        .get(&fact.object.card)
                        .is_some_and(|c| c.owner == fact.owner)
                    && matches!(damage.recipient, DamageRecipient::Player { player } if player.0 < 4)
                    && matches!(pattern, EventPattern::DealsDamage { source: pred, combat, recipient }
                    if (!*combat || *kind == DamageKind::Combat)
                        && matches!(damage.recipient, DamageRecipient::Player { player }
                            if player_in(recipient, Some(player), controller))
                        && match match_source {
                            Some(appearance) => pred.reads_characteristics()
                                && appearance.zone == Zone::Battlefield
                                && holds(pred, Some(&frozen(appearance)), controller,
                                    fact.object, &identity(fact.object)),
                            None => !pred.reads_characteristics()
                                && identity_holds(pred, fact.object, &identity(fact.object)),
                        })
                    && death_controller.is_none()
                // No live source/recipient or current combat requirement. CR
                // 113.7a/800.4 preserve a surviving controller's invocation.
                // Accepted-input replay proves actual member/kind/final amount
                // and the separately frozen match appearance, including coherent
                // rehashed lies that satisfy these guards.
            }
            TriggerEventWitness::CreatureAttacked {
                attacker,
                defender_player_id,
                ..
            } => {
                // CR 603.3a: the controller of the attacking source at the
                // event controls its own attack trigger.
                if attacker.object == source && attacker.controller != controller {
                    return Err("snapshot invalid self-attack trigger controller".into());
                }
                self.validate_fact_identity(attacker)?;
                attacker.zone == Zone::Battlefield
                    && !attacker.tapped
                    && (attacker.attack_ready || attacker.keywords.contains(&KeywordAbility::Haste))
                    && !attacker.keywords.contains(&KeywordAbility::Defender)
                    && attacker.controller.0 < 4
                    && attacker.default_controller.0 < 4
                    && defender_player_id.0 < 4
                    && *defender_player_id != attacker.controller
                    && self
                        .state
                        .rules
                        .cards
                        .get(&attacker.object.card)
                        .is_some_and(|c| c.owner == attacker.owner)
                    && attacker.characteristics.has_type(CardKind::Creature)
                    && matches!(pattern, EventPattern::Attacks { attacker: pred }
                        if holds(pred, Some(&frozen(attacker)), controller, attacker.object,
                            &identity(attacker.object)))
                    && death_controller.is_none()
                // Deliberately no current combat/player-liveness requirement:
                // CR 113.7a/800.4 preserve a surviving controller's invocation.
                // Accepted-input replay proves the actual declaration/facts.
            }
            TriggerEventWitness::BecameTarget {
                target,
                target_controller_live,
                actor,
                is_spell,
                controller_at_match,
            } => {
                *controller_at_match == controller
                    && self.valid_target_match(
                        source,
                        controller,
                        pattern,
                        Frozen {
                            live: *target_controller_live,
                            ..frozen(target)
                        },
                        *actor,
                        *is_spell,
                    )?
            }
            TriggerEventWitness::AttackerUnblocked {
                attacker,
                defender,
                controller_at_match,
            } => {
                self.validate_fact_identity(attacker)?;
                *controller_at_match == controller
                    && attacker.zone == Zone::Battlefield
                    && attacker.characteristics.has_type(CardKind::Creature)
                    && self.state.rules.original_seating.contains(defender)
                    && self
                        .state
                        .rules
                        .original_seating
                        .contains(&attacker.controller)
                    && self
                        .state
                        .rules
                        .original_seating
                        .contains(&attacker.default_controller)
                    && *defender != attacker.controller
                    && (attacker.object != source || attacker.controller == controller)
                    && self
                        .state
                        .rules
                        .cards
                        .get(&attacker.object.card)
                        .is_some_and(|card| card.owner == attacker.owner)
                    && matches!(pattern, EventPattern::AttacksUnblocked { attacker: pred }
                        if holds(pred, Some(&frozen(attacker)), controller, attacker.object, &identity(attacker.object)))
            }
            TriggerEventWitness::CardDrawn {
                player,
                controller_at_match,
                nth,
            } => {
                *controller_at_match == controller
                    && self.state.rules.original_seating.contains(player)
                    && ordinal_evidence(pattern, *nth)
                    && matches!(pattern, EventPattern::Draws { player: expected, .. }
                        if player_in(expected, Some(*player), controller))
            }
            // CR 119.9: replay proves the actual gain and its turn count.
            TriggerEventWitness::LifeGained {
                player,
                amount,
                controller_at_match,
                nth,
            } => {
                *controller_at_match == controller
                    && *amount > Quantity::zero()
                    && self.state.rules.original_seating.contains(player)
                    && ordinal_evidence(pattern, *nth)
                    && matches!(pattern, EventPattern::GainsLife { player: expected, .. }
                        if player_in(expected, Some(*player), controller))
                    && death_controller.is_none()
            }
            TriggerEventWitness::Discarded {
                player,
                movement,
                controller_at_match,
            } => {
                *controller_at_match == controller
                    && self.state.rules.original_seating.contains(player)
                    && movement.from == (Zone::Hand { owner: *player })
                    && matches!(movement.destination, Zone::Graveyard { .. })
                    && matches!(pattern, EventPattern::Discards { player: expected }
                        if player_in(expected, Some(*player), controller))
                    && death_controller.is_none()
            }
            TriggerEventWitness::Blocked {
                blocker: facts,
                controller_at_match,
            }
            | TriggerEventWitness::BecameBlocked {
                attacker: facts,
                controller_at_match,
            }
            | TriggerEventWitness::PermanentDamaged {
                recipient: facts,
                controller_at_match,
                ..
            } => {
                self.validate_fact_identity(facts)?;
                let matched = match (witness, pattern) {
                    (
                        TriggerEventWitness::Blocked { .. },
                        EventPattern::Blocks { blocker: pred },
                    )
                    | (
                        TriggerEventWitness::BecameBlocked { .. },
                        EventPattern::BecomesBlocked { attacker: pred },
                    ) => {
                        facts.characteristics.has_type(CardKind::Creature)
                            && holds(
                                pred,
                                Some(&frozen(facts)),
                                controller,
                                facts.object,
                                &identity(facts.object),
                            )
                    }
                    (
                        TriggerEventWitness::PermanentDamaged { kind, .. },
                        EventPattern::IsDealtDamage {
                            object: pred,
                            combat,
                        },
                    ) => {
                        (!*combat || *kind == DamageKind::Combat)
                            && holds(
                                pred,
                                Some(&frozen(facts)),
                                controller,
                                facts.object,
                                &identity(facts.object),
                            )
                    }
                    _ => false,
                };
                matched
                    && *controller_at_match == controller
                    && facts.zone == Zone::Battlefield
                    && self
                        .state
                        .rules
                        .original_seating
                        .contains(&facts.controller)
                    && self
                        .state
                        .rules
                        .cards
                        .get(&facts.object.card)
                        .is_some_and(|c| c.owner == facts.owner)
                    && death_controller.is_none()
            }
            TriggerEventWitness::Sacrificed {
                player,
                object,
                controller_at_match,
            } => {
                self.validate_fact_identity(object)?;
                *controller_at_match == controller
                    && object.zone == Zone::Battlefield
                    && object.controller == *player
                    && self.state.rules.original_seating.contains(player)
                    && self
                        .state
                        .rules
                        .cards
                        .get(&object.object.card)
                        .is_some_and(|c| c.owner == object.owner)
                    && matches!(pattern, EventPattern::Sacrifices { player: expected, object: pred }
                        if player_in(expected, Some(*player), controller)
                            && holds(pred, Some(&frozen(object)), controller, object.object,
                                &identity(object.object)))
                    && death_controller.is_none()
            }
            TriggerEventWitness::SpellCast { cast } => {
                self.valid_cast_facts(cast)?
                    && ordinal_evidence(pattern, cast.nth)
                    && matches!(pattern, EventPattern::Cast { caster, spell, targets_source, .. }
                        if player_in(caster, Some(cast.caster), controller)
                            && (!*targets_source || cast.targets.contains(&source))
                            && holds(spell, Some(&Frozen { facts: &cast.spell, token: false,
                                live: true }), controller,
                                cast.spell.object, &identity(cast.spell.object)))
                    && death_controller.is_none()
            }

            TriggerEventWitness::StepBegan {
                turn,
                owner,
                active,
            } => {
                turn.number > 0
                    && turn.number <= self.state.rules.turn.number
                    && owner.0 < 4
                    && active.is_none_or(|p| p == *owner)
                    && matches!(pattern, EventPattern::StepBegins { step, active: player }
                        if step_matches(*step, turn) && player_in(player, *active, controller))
                    && death_controller.is_none()
            }
            TriggerEventWitness::TokenEntered { object } => {
                self.state
                    .rules
                    .tokens
                    .get(&object.card)
                    .is_some_and(|t| !t.is_copy())
                    && object.incarnation == 0
                    && matches!(pattern, EventPattern::Enters { object: pred }
                        if !pred.reads_characteristics() && !binds_object
                            && identity_holds(pred, *object, &identity(*object)))
                    && death_controller.is_none()
            }
            TriggerEventWitness::Tapped {
                object,
                controller: at_tap,
            } => {
                matches!(pattern, EventPattern::BecomesTapped { object: pred }
                    if pred.is_ref(Ref::Source) && identity_holds(pred, *object, &identity(*object)))
                    && *object == source
                    && *at_tap == controller
                    && death_controller.is_none()
            }
            TriggerEventWitness::TurnedFaceUp {
                object,
                controller: at_turn,
            } => {
                matches!(pattern, EventPattern::TurnedFaceUp { object: pred }
                    if pred.is_ref(Ref::Source) && identity_holds(pred, *object, &identity(*object)))
                    && *object == source
                    && *at_turn == controller
                    && death_controller.is_none()
            }
            TriggerEventWitness::Entered {
                entrant, movement, ..
            } => {
                death_controller.is_none()
                    && self.valid_entry_subject(entrant, movement.as_ref())?
                    && entrant.zone == Zone::Battlefield
                    && matches!(pattern, EventPattern::Enters { object: pred }
                        if (pred.reads_characteristics() || binds_object)
                            && (entrant.object != source || entrant.controller == controller)
                            && holds(pred, Some(&frozen(entrant)), controller, entrant.object,
                                &identity(entrant.object)))
            }
            TriggerEventWitness::Moved { movement: m } => {
                let subject = if witness.enters() {
                    m.successor
                } else {
                    m.previous
                };
                self.state.rules.cards.contains_key(&m.previous.card)
                    && m.successor.card == m.previous.card
                    && m.previous.incarnation.checked_add(1) == Some(m.successor.incarnation)
                    && (match pattern {
                        EventPattern::Dies { object: pred } => {
                            witness.dies()
                                && self
                                    .state
                                    .rules
                                    .cards
                                    .get(&subject.card)
                                    .is_some_and(|card| {
                                        m.destination == (Zone::Graveyard { owner: card.owner })
                                    })
                                && identity_holds(pred, subject, &identity(subject))
                                && creature_card(pred, subject)?
                        }
                        // CR 603.6c: replay proves the departed facts.
                        EventPattern::LeavesBattlefield { object: pred } => {
                            witness.leaves() && identity_holds(pred, subject, &identity(subject))
                        }
                        // An entry whose pattern read characteristics
                        // retains them as `Entered`.
                        EventPattern::Enters { object: pred } => {
                            witness.enters()
                                && !pred.reads_characteristics()
                                && !binds_object
                                && identity_holds(pred, subject, &identity(subject))
                        }
                        _ => false,
                    })
            }
        };
        if !valid {
            return Err("snapshot trigger event witness invariant".into());
        }
        Ok(())
    }
    fn valid_trigger_occurrence(
        &self,
        event: &TriggerOccurrenceId,
        witness: &TriggerEventWitness,
    ) -> bool {
        match (event, witness) {
            (TriggerOccurrenceId::Cast(id), TriggerEventWitness::SpellCast { cast }) => {
                id.cast == cast.stack_id && id.cast.0 < self.state.rules.next_stack_id
            }
            (
                TriggerOccurrenceId::Target(id),
                TriggerEventWitness::BecameTarget {
                    target,
                    actor,
                    is_spell,
                    ..
                },
            ) => {
                id.targeting.0 < self.state.rules.next_stack_id
                    && self.entry(id.targeting).map_or(true, |entry| {
                        entry.controller == *actor
                            && entry.kind.is_spell() == *is_spell
                            && Self::targeted_permanents(entry).contains(&target.object)
                            && self.stack_entry_complete(entry)
                    })
            }
            (TriggerOccurrenceId::Batch(id), w) => {
                !matches!(
                    w,
                    TriggerEventWitness::SpellCast { .. }
                        | TriggerEventWitness::BecameTarget { .. }
                ) && id.batch < self.state.rules.next_batch_id
            }
            _ => false,
        }
    }

    fn validate_trigger_occurrence<'a>(
        source: ObjectId,
        face: CardFace,
        ability: EffectiveTriggerAbility,
        event: &'a TriggerOccurrenceId,
        witness: &TriggerEventWitness,
        seen: &mut TriggerOccurrences<'a>,
    ) -> Result<(), String> {
        if matches!(ability, EffectiveTriggerAbility::Followup { .. }) {
            return Ok(());
        }
        let subject = witness.subject();
        let damage_member = match witness {
            TriggerEventWitness::DamageDealt { damage, .. } => Some(damage.id),
            _ => None,
        };
        if !seen.members.insert(TriggerMember {
            source,
            face,
            ability,
            event,
            subject,
            damage: damage_member,
        }) {
            return Err("snapshot duplicate trigger occurrence".into());
        }
        // CR 400.7, 603.2c: a particular incarnation enters once and dies
        // once, even if a saved trigger gives it a second occurrence ID.
        let movement = if witness.enters() {
            Some(TriggerMovement::Entry)
        } else if witness.dies() {
            Some(TriggerMovement::Death)
        } else {
            None
        };
        if let Some(movement) = movement
            && let Some(subject) = subject
            && !seen
                .movements
                .insert((source, face, ability, subject, movement))
        {
            return Err("snapshot duplicate trigger movement".into());
        }
        Ok(())
    }
    pub(crate) fn validate_trigger_entry(
        &self,
        entry: &StackInstance,
    ) -> Result<Option<TriggerId>, String> {
        let StackKind::Triggered {
            ability,

            source_handles,
            trigger_id,
            event,
            witness,
            match_condition,
        } = &entry.kind
        else {
            return Ok(None);
        };
        self.validate_trigger_source(
            entry.source,
            entry.program_face(),
            *ability,
            entry.controller,
            entry
                .source_facts
                .as_ref()
                .ok_or("snapshot missing triggered source facts")?,
            source_handles,
        )?;
        self.validate_match_condition(
            entry.source,
            entry.program_face(),
            *ability,
            entry.controller,
            match_condition.as_ref(),
            witness,
        )?;
        if let EffectiveTriggerAbility::Followup { creator } = *ability {
            self.validate_resolution_trigger_invocation(
                creator,
                entry.source,
                entry.controller,
                *trigger_id,
                event,
                witness,
            )?;
            if entry.death_controller.is_some() {
                return Err("resolution trigger has death controller".into());
            }
        } else {
            self.validate_trigger_witness(
                entry.source,
                entry.program_face(),
                *ability,
                entry.controller,
                entry.death_controller,
                witness,
            )?;
        }
        if trigger_id.0 >= self.state.rules.next_trigger_id
            || !self.valid_trigger_occurrence(event, witness)
            || entry.cast_from
                != entry
                    .source_facts
                    .as_ref()
                    .ok_or("missing trigger facts")?
                    .zone
            || (!entry.targets.is_empty()
                && self.program_for(entry)?.targets.first().is_none_or(|t| {
                    !matches!(
                        t.domain,
                        TargetDomain::BattlefieldPermanent
                            | TargetDomain::BattlefieldCreature
                            | TargetDomain::ControlledBattlefieldPermanent
                            | TargetDomain::ControlledBattlefieldCreature
                            | TargetDomain::OwnGraveyardCreatureCard
                            | TargetDomain::GraveyardCard { .. }
                            | TargetDomain::OwnGraveyardCard
                    )
                }))
            || !entry.stack_targets.is_empty()
            || entry.total_cost
                != (ManaCost {
                    generic: Quantity::zero(),
                    colored: vec![],
                })
        {
            return Err("snapshot triggered invocation invariant".into());
        }
        Ok(Some(*trigger_id))
    }
    pub(crate) fn validate_trigger_state(&self) -> Result<(), String> {
        if self.state.rules.pending_triggers.len() + self.state.rules.stack.len()
            > self.stack_limit()
        {
            return Err("snapshot trigger/stack resource bound".into());
        }
        let mut ids = BTreeSet::new();
        let mut occurrences = TriggerOccurrences::default();
        for trigger in &self.state.rules.pending_triggers {
            self.validate_trigger_source(
                trigger.source,
                trigger.source_face,
                trigger.ability,
                trigger.controller,
                &trigger.source_facts,
                &trigger.source_handles,
            )?;
            self.validate_match_condition(
                trigger.source,
                trigger.source_face,
                trigger.ability,
                trigger.controller,
                trigger.match_condition.as_ref(),
                &trigger.witness,
            )?;
            if !self.is_live(trigger.controller)
                || trigger.id.0 >= self.state.rules.next_trigger_id
                || !ids.insert(trigger.id)
                || !self.valid_trigger_occurrence(&trigger.event, &trigger.witness)
            {
                return Err("snapshot pending trigger identity invariant".into());
            }
            if let EffectiveTriggerAbility::Followup { creator } = trigger.ability {
                self.validate_resolution_trigger_invocation(
                    creator,
                    trigger.source,
                    trigger.controller,
                    trigger.id,
                    &trigger.event,
                    &trigger.witness,
                )?;
                if trigger.death_controller.is_some() {
                    return Err("resolution trigger has death controller".into());
                }
            } else {
                self.validate_trigger_witness(
                    trigger.source,
                    trigger.source_face,
                    trigger.ability,
                    trigger.controller,
                    trigger.death_controller,
                    &trigger.witness,
                )?;
            }
            Self::validate_trigger_occurrence(
                trigger.source,
                trigger.source_face,
                trigger.ability,
                &trigger.event,
                &trigger.witness,
                &mut occurrences,
            )?;
        }
        for entry in self
            .state
            .rules
            .stack
            .iter()
            .chain(self.state.rules.detached.values())
        {
            if let Some(id) = self.validate_trigger_entry(entry)?
                && !ids.insert(id)
            {
                return Err("snapshot duplicate trigger identity".into());
            }
            if let StackKind::Triggered {
                ability,
                event,
                witness,
                ..
            } = &entry.kind
            {
                Self::validate_trigger_occurrence(
                    entry.source,
                    entry.program_face(),
                    *ability,
                    event,
                    witness,
                    &mut occurrences,
                )?;
            }
        }
        if let Frame::TriggerOrder { placement } | Frame::TriggerTargets { placement, .. } =
            &self.state.rules.frame
        {
            if placement.before > self.state.rules.next_trigger_id
                || placement.remaining.iter().any(|p| !self.is_live(*p))
                || placement.remaining.iter().collect::<BTreeSet<_>>().len()
                    != placement.remaining.len()
                || placement.order.iter().collect::<BTreeSet<_>>().len() != placement.order.len()
                || placement.order.iter().any(|id| {
                    !self.state.rules.pending_triggers.iter().any(|t| {
                        t.id == *id
                            && t.id.0 < placement.before
                            && Some(&t.controller) == placement.remaining.first()
                    })
                })
            {
                return Err("snapshot APNAP placement invariant".into());
            }
            if let Frame::TriggerTargets { stack_id, .. } = &self.state.rules.frame {
                let top = self.top()?;
                if top.id != *stack_id
                    || !matches!(top.kind, StackKind::Triggered { .. })
                    || top.paid.is_some()
                    || self.bound_target_count(top) != 0
                    || Some(&top.controller) != placement.remaining.first()
                {
                    return Err("snapshot triggered target frame invariant".into());
                }
            }
        }
        Ok(())
    }
}

impl RulesTxn {
    fn bind_trigger_entity_targets(
        &mut self,
        placement: TriggerPlacement,
        stack_id: StackId,
    ) -> Result<(), String> {
        let input = self
            .rules_mut()
            .pending
            .take()
            .ok_or("missing trigger entity targets")?;
        let entities = input.entities()?;
        self.validate_entity_targets(input.player, &entities)?;
        let announced = (0..4)
            .map(|v| {
                entities
                    .iter()
                    .map(|t| match t {
                        TargetEntity::Player { .. } => Ok(None),
                        TargetEntity::Object { object } => self
                            .object_handle_id(PlayerId(v), *object)
                            .cloned()
                            .map(Some),
                    })
                    .collect::<Result<Vec<_>, String>>()
            })
            .collect::<Result<Vec<_>, _>>()?;
        let top = self
            .rules_mut()
            .stack
            .last_mut()
            .ok_or("missing triggered invocation")?;
        if top.id != stack_id {
            return Err("trigger identity".into());
        }
        let StackKind::Triggered { trigger_id, .. } = top.kind else {
            return Err("trigger kind".into());
        };
        top.entity_targets = entities;
        top.announced_entity_targets = announced;
        top.paid = Some(top.total_cost.clone());
        let player = top.controller;
        self.record_event(Event::TriggerPlaced {
            trigger_id,
            stack_id,
            player,
        })?;
        self.stage_target_triggers()?;
        self.finish_target_triggers(stack_id)?;
        self.rules_mut().frame = Frame::TriggerOrder { placement };
        Ok(())
    }
    pub(super) fn reject_trigger_placement(
        &mut self,
        placement: TriggerPlacement,
        stack_id: StackId,
        reason: TriggerPlacementFailure,
    ) -> Result<(), String> {
        let top = self.top()?;
        let StackKind::Triggered { trigger_id, .. } = top.kind else {
            return Err("rejected placement is not a trigger".into());
        };
        if top.id != stack_id {
            return Err("rejected placement is not the top trigger".into());
        }
        self.rules_mut().stack.pop();
        self.record_event(Event::TriggeredAbilityNotPlaced {
            trigger_id,
            stack_id,
            reason,
        })?;
        self.rules_mut().frame = Frame::TriggerOrder { placement };
        Ok(())
    }

    pub(super) fn bind_trigger_object_targets(
        &mut self,
        placement: TriggerPlacement,
        stack_id: StackId,
    ) -> Result<(), String> {
        let input = self
            .rules_mut()
            .pending
            .take()
            .ok_or("missing creature-trigger targets")?;

        let objects = input.objects()?;
        let announced = (0..4)
            .map(|v| {
                objects
                    .iter()
                    .map(|o| self.object_handle_id(PlayerId(v), *o).cloned())
                    .collect::<Result<Vec<_>, _>>()
            })
            .collect::<Result<Vec<_>, _>>()?;
        let top = self
            .rules_mut()
            .stack
            .last_mut()
            .ok_or("missing targeted creature trigger")?;
        if top.id != stack_id {
            return Err("wrong targeted creature trigger".into());
        }
        let StackKind::Triggered { trigger_id, .. } = top.kind else {
            return Err("wrong creature trigger kind".into());
        };
        top.targets = objects;
        top.announced_targets = announced;
        top.paid = Some(top.total_cost.clone());
        let player = top.controller;
        self.record_event(Event::TriggerPlaced {
            trigger_id,
            stack_id,
            player,
        })?;
        self.stage_target_triggers()?;
        self.finish_target_triggers(stack_id)?;
        self.rules_mut().frame = Frame::TriggerOrder { placement };
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "origin", rename_all = "snake_case", deny_unknown_fields)]
pub enum EffectiveTriggerAbility {
    Intrinsic { index: u32 },
    Followup { creator: StackId },
}

impl RulesTxn {
    pub(super) fn effective_trigger(
        &self,
        source: ObjectId,
        face: CardFace,
        ability: EffectiveTriggerAbility,
    ) -> Result<std::borrow::Cow<'_, mtg_contracts::TriggeredAbility>, String> {
        match ability {
            EffectiveTriggerAbility::Intrinsic { index } => self
                .face_definition(source.card, face)?
                .triggered
                .get(index as usize)
                .map(std::borrow::Cow::Borrowed)
                .ok_or_else(|| "missing intrinsic triggered definition".into()),
            EffectiveTriggerAbility::Followup { .. } => {
                Err("follow-up trigger has no intrinsic definition".into())
            }
        }
    }
}

fn trigger_history_charge(witness: &TriggerEventWitness) -> usize {
    match witness {
        TriggerEventWitness::SpellCast { cast } => stored_history::payload_charge(&cast.history),
        _ => 0,
    }
}
pub fn entry_history_charge(entry: &StackInstance) -> usize {
    match &entry.kind {
        StackKind::Triggered { witness, .. } => trigger_history_charge(witness),
        _ => 0,
    }
}
impl RulesTxn {
    pub(crate) fn trigger_payload_charge(&self) -> usize {
        let mut charge = self.resolution_trigger_history_charge();
        // Cascade/owned-child frames already charge their complete payload.
        for t in &self.state.rules.pending_triggers {
            charge = charge.saturating_add(trigger_history_charge(&t.witness));
        }
        let rules = &self.state.rules;
        for s in rules.stack.iter().chain(rules.detached.values()) {
            charge = charge.saturating_add(entry_history_charge(s));
        }

        charge
    }
}
