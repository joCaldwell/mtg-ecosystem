//! Canonical actuals (E1): one typed record per member of each rules
//! occurrence, written once at publication and read by history, trigger
//! matching, delayed watchers and mana reversal. Creation, battlefield entry
//! zone movement and counters are migrated. The remaining
//! families keep their `EventOutcome` boundaries until E3.
//! The runtime invariants in `docs/game-engine-mechanics.md` own the
//! contract. CR 111.1-2, 111.13, 400.7, 603.2c, 603.6a, 603.10a, 700.4,
//! 733.1: <https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt>
use super::*;
use mtg_contracts::kir::CounterKind;
use tokens::TokenTemplate;
use triggers::TurnCastHistory;

/// One semantic member of an occurrence. Member indices carry no public
/// order; they only distinguish simultaneous members of one occurrence.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventId {
    pub occurrence: OccurrenceId,
    pub member: u32,
}

/// The `Simultaneous` group node whose members share one common prior.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct GroupId {
    pub batch: u64,
    pub path: Vec<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum FacetKind {
    Moved,
    Created,
    Entered,
    Counters,
}

/// One facet of one event; it resolves against the published actuals, with
/// or without tracing.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ActualRef {
    pub event: EventId,
    pub facet: FacetKind,
}

/// What a cost paid for (CR 118.11, 606.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CostOf {
    Spell,
    Activation,
    ManaAbility,
    Loyalty,
    Resolution,
    Entry,
}

/// The root that answers cost accounting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Root {
    Cost(CostOf),
    Effect,
    TurnBased,
    StateBased,
    SpecialAction,
    Rule,
}

/// The resolving instruction an effect-rooted actual came from. Stage 1's
/// resolving-frame `StepInstance` replaces it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StepRef {
    pub invocation: StackId,
    pub cursor: u32,
    pub block: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Origin {
    pub root: Root,
    pub invocation: Option<StackId>,
    pub step: Option<StepRef>,
    /// The payment term within an ordered cost (E2).
    pub term: Option<u32>,
}

/// One applied replacement instance (CR 614.5, 616.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReplacementKey {
    pub source: ObjectId,
    pub ability: u32,
}

/// The proposal, its original fact and the replacements applied in order;
/// the last application is the immediate producer (CR 614.16). E2's
/// pre-commit protocol records it; E1 actuals carry none.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lineage {
    pub proposal: OccurrenceId,
    pub original: Box<ProposedEvent>,
    pub applied: Vec<ReplacementKey>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Destination {
    Zone(Zone),
    /// CR 800.4a: the object left the game with its departing owner.
    LeftGame {
        owner: PlayerId,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoveKind {
    /// Put into the destination by an effect, cost, action or rule.
    Put,
    /// CR 608.3: a resolving permanent spell, or its copy, became a permanent.
    Resolved { stack_id: StackId },
}

/// CR 122.1: a counter belongs to a particular incarnation or player.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "holder", rename_all = "snake_case", deny_unknown_fields)]
pub enum CounterHolder {
    Permanent {
        object: ObjectId,
        controller: PlayerId,
    },
    Player {
        player: PlayerId,
    },
}

/// The settled change of one kind on one holder. The prior is zero on
/// entry (CR 122.6, 400.7); the entry link names the new incarnation's event.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CounterChange {
    pub holder: CounterHolder,
    pub counter: CounterKind,
    pub previous: Quantity,
    pub current: Quantity,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placed_by: Option<PlayerId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub with_entry: Option<EventId>,
}

impl CounterChange {
    pub fn delta(&self) -> Quantity {
        let mut delta = self.current.clone();
        delta.subtract(&self.previous);
        delta
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Facet {
    /// A zone change. `prior` retains the complete row of a permanent that
    /// left the battlefield (CR 603.10a); `successor` is the new incarnation
    /// (CR 400.7), absent when the object left the game.
    Moved {
        object: ObjectId,
        prior: Option<Box<ObjectBefore>>,
        from: Zone,
        to: Destination,
        successor: Option<ObjectId>,
        how: MoveKind,
    },
    /// CR 111.1-2: a token the creating player created. A resolving
    /// permanent-spell copy becomes a token without being created (CR 111.13).
    Created {
        creator: PlayerId,
        recipe: Box<TokenTemplate>,
        invocation: Option<StackId>,
    },
    /// CR 603.6a: the resulting battlefield incarnation.
    Entered {
        object: ObjectId,
        controller: PlayerId,
        tapped: bool,
        token: bool,
        attached_to: Option<ObjectId>,
        face: CardFace,
    },
    Counters(CounterChange),
}
impl Facet {
    pub fn kind(&self) -> FacetKind {
        match self {
            Self::Moved { .. } => FacetKind::Moved,
            Self::Created { .. } => FacetKind::Created,
            Self::Entered { .. } => FacetKind::Entered,
            Self::Counters(_) => FacetKind::Counters,
        }
    }
}

/// One member of a rules occurrence, with at most one facet per kind, so an
/// entry counts once whether it arrived by move or by creation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Actual {
    pub id: EventId,
    pub group: Option<GroupId>,
    pub cause: BatchCause,
    pub origin: Origin,
    pub lineage: Option<Lineage>,
    pub facets: Vec<Facet>,
}
impl Actual {
    pub fn counters(&self) -> Option<&CounterChange> {
        match self.facet(FacetKind::Counters) {
            Some(Facet::Counters(change)) => Some(change),
            _ => None,
        }
    }

    pub fn facet(&self, kind: FacetKind) -> Option<&Facet> {
        self.facets.iter().find(|facet| facet.kind() == kind)
    }
    pub fn reference(&self, kind: FacetKind) -> Option<ActualRef> {
        self.facet(kind).map(|_| ActualRef {
            event: self.id.clone(),
            facet: kind,
        })
    }
    /// CR 614.16: an effect root, or any applied replacement.
    pub fn produced_by_effect(&self) -> bool {
        self.origin.root == Root::Effect
            || self
                .lineage
                .as_ref()
                .is_some_and(|lineage| !lineage.applied.is_empty())
    }
    /// The entered incarnation, when this member entered the battlefield.
    pub fn entrant(&self) -> Option<ObjectId> {
        match self.facet(FacetKind::Entered)? {
            Facet::Entered { object, .. } => Some(*object),
            _ => None,
        }
    }
    /// The prior row of a permanent that left the battlefield, and whether
    /// it was put into a graveyard (CR 700.4).
    pub fn departure(&self) -> Option<(&ObjectBefore, bool)> {
        match self.facet(FacetKind::Moved)? {
            Facet::Moved {
                prior: Some(prior),
                from: Zone::Battlefield,
                to,
                ..
            } if *to != Destination::Zone(Zone::Battlefield) => Some((
                prior,
                matches!(to, Destination::Zone(Zone::Graveyard { .. })),
            )),
            _ => None,
        }
    }
    /// The legacy zone-change view of the move, for readers of trigger
    /// witnesses (`TriggerEventWitness::Moved`).
    pub fn movement(&self) -> Option<MoveOutcome> {
        match self.facet(FacetKind::Moved)? {
            Facet::Moved {
                object,
                from,
                to: Destination::Zone(destination),
                successor: Some(successor),
                ..
            } => Some(MoveOutcome {
                previous: *object,
                successor: *successor,
                from: *from,
                destination: *destination,
            }),
            _ => None,
        }
    }
}

impl ActualRef {
    /// Resolve against published actuals, with or without tracing.
    pub fn resolve<'a>(&self, actuals: &'a [Actual]) -> Option<&'a Facet> {
        actuals
            .iter()
            .find(|actual| actual.id == self.event)?
            .facet(self.facet)
    }
}

/// One seat's change to the per-turn ledger: counts added, and flags this
/// change first set relative to the ledger before it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SeatDelta {
    pub seat: PlayerId,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub permanent_left: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub creature_died: bool,
    #[serde(default, skip_serializing_if = "state::is_zero_count")]
    pub artifacts_entered: u32,
    #[serde(default, skip_serializing_if = "state::is_zero_count")]
    pub creatures_entered: u32,
    #[serde(default, skip_serializing_if = "state::is_zero_count")]
    pub nonlands_entered: u32,
    #[serde(default, skip_serializing_if = "state::is_zero_count")]
    pub tokens_created: u32,
}
impl SeatDelta {
    fn new(seat: PlayerId) -> Self {
        Self {
            seat,
            permanent_left: false,
            creature_died: false,
            artifacts_entered: 0,
            creatures_entered: 0,
            nonlands_entered: 0,
            tokens_created: 0,
        }
    }
    fn is_empty(&self) -> bool {
        *self == Self::new(self.seat)
    }
}

fn q(n: u32) -> Quantity {
    Quantity::from(i64::from(n))
}

/// The history reducer's result for one publication, or the fold of
/// several. Its exact inverse undoes it (CR 733.1).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LedgerDelta {
    /// One row per changed seat, in seat order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub seats: Vec<SeatDelta>,
}
impl LedgerDelta {
    pub fn is_empty(&self) -> bool {
        self.seats.is_empty()
    }
    fn seat(&mut self, seat: PlayerId) -> &mut SeatDelta {
        let at = match self.seats.binary_search_by_key(&seat, |row| row.seat) {
            Ok(at) => at,
            Err(at) => {
                self.seats.insert(at, SeatDelta::new(seat));
                at
            }
        };
        &mut self.seats[at]
    }
    fn count(n: &mut u32) -> Result<(), String> {
        *n = n.checked_add(1).ok_or("turn history count limit")?;
        Ok(())
    }
    /// Fold a later delta into this one.
    pub(super) fn merge(&mut self, later: &LedgerDelta) -> Result<(), String> {
        for row in &later.seats {
            let into = self.seat(row.seat);
            into.permanent_left |= row.permanent_left;
            into.creature_died |= row.creature_died;
            for (a, b) in [
                (&mut into.artifacts_entered, row.artifacts_entered),
                (&mut into.creatures_entered, row.creatures_entered),
                (&mut into.nonlands_entered, row.nonlands_entered),
                (&mut into.tokens_created, row.tokens_created),
            ] {
                *a = a.checked_add(b).ok_or("turn history count limit")?;
            }
        }
        Ok(())
    }
    pub(super) fn apply(&self, history: &mut [TurnCastHistory; 4]) -> Result<(), String> {
        for row in &self.seats {
            let h = history
                .get_mut(row.seat.0 as usize)
                .ok_or("invalid turn history seat")?;
            h.permanent_left |= row.permanent_left;
            h.creature_died |= row.creature_died;
            h.artifacts_entered.add(&q(row.artifacts_entered));
            h.creatures_entered.add(&q(row.creatures_entered));
            h.nonlands_entered.add(&q(row.nonlands_entered));
            h.tokens_created = h
                .tokens_created
                .checked_add(row.tokens_created)
                .ok_or("token history count limit")?;
        }
        Ok(())
    }
    /// CR 733.1: the exact inverse. Every fact this delta recorded must
    /// still hold.
    pub(super) fn revert(&self, history: &mut [TurnCastHistory; 4]) -> Result<(), String> {
        self.check_holds(history)?;
        for row in &self.seats {
            let h = &mut history[row.seat.0 as usize];
            h.permanent_left &= !row.permanent_left;
            h.creature_died &= !row.creature_died;
            h.artifacts_entered.subtract(&q(row.artifacts_entered));
            h.creatures_entered.subtract(&q(row.creatures_entered));
            h.nonlands_entered.subtract(&q(row.nonlands_entered));
            h.tokens_created -= row.tokens_created;
        }
        Ok(())
    }
    /// Seats are distinct, rows non-empty and every recorded fact holds.
    pub(super) fn check_holds(&self, history: &[TurnCastHistory; 4]) -> Result<(), String> {
        let le = |n: u32, have: &Quantity| q(n) <= *have;
        if !self.seats.windows(2).all(|w| w[0].seat < w[1].seat) {
            return Err("invalid ledger delta seats".into());
        }
        for row in &self.seats {
            let h = history
                .get(row.seat.0 as usize)
                .ok_or("invalid ledger delta seat")?;
            if row.is_empty()
                || row.permanent_left && !h.permanent_left
                || row.creature_died && !h.creature_died
                || !le(row.artifacts_entered, &h.artifacts_entered)
                || !le(row.creatures_entered, &h.creatures_entered)
                || !le(row.nonlands_entered, &h.nonlands_entered)
                || row.tokens_created > h.tokens_created
            {
                return Err("invalid ledger delta".into());
            }
        }
        Ok(())
    }
}

/// A recorder for the actuals one bounded procedure publishes (a mana
/// ability's costs), never state: the receipt keeps what it needs.
#[derive(Clone, Debug, Default)]
pub struct ActualJournal {
    /// Every published member, and one member-0 identity for each occurrence
    /// of a family still on its `EventOutcome` boundary.
    pub events: Vec<EventId>,
    pub actuals: Vec<Actual>,
    pub ledger: LedgerDelta,
}

impl Origin {
    fn of(cause: &BatchCause, state: &RulesTxn) -> Self {
        use BatchCause as C;
        let (root, invocation) = match cause {
            C::AbilityActivation { stack_id, .. } => {
                (Root::Cost(CostOf::Activation), Some(*stack_id))
            }
            C::ManaActivation { .. } => (Root::Cost(CostOf::ManaAbility), None),
            C::Payment { stack_id, .. } => (
                Root::Cost(
                    if state
                        .state
                        .rules
                        .stack
                        .iter()
                        .any(|s| s.id == *stack_id && s.kind.is_spell())
                    {
                        CostOf::Spell
                    } else {
                        CostOf::Activation
                    },
                ),
                Some(*stack_id),
            ),
            C::Instruction { stack_id, .. } | C::InstructionDraw { stack_id, .. } => {
                (Root::Effect, Some(*stack_id))
            }
            C::Combat | C::Cleanup | C::TurnProgression | C::UntapStep | C::PrecombatMainLore => {
                (Root::TurnBased, None)
            }
            C::DrawStep { .. } => (Root::TurnBased, None),
            C::StateBased | C::DeathtouchCheck | C::CommanderReturn => (Root::StateBased, None),
            C::LandPlay { .. } | C::SpecialAction { .. } => (Root::SpecialAction, None),
            C::Casting { stack_id, .. }
            | C::CastReversal { stack_id, .. }
            | C::SpellCompletion { stack_id, .. } => (Root::Rule, Some(*stack_id)),
            C::Setup | C::Departure { .. } | C::ControlReconciliation | C::Fixture => {
                (Root::Rule, None)
            }
        };
        let step = match cause {
            C::Instruction {
                stack_id,
                cursor,
                source_block,
                ..
            }
            | C::InstructionDraw {
                stack_id,
                cursor,
                source_block,
                ..
            } => Some(StepRef {
                invocation: *stack_id,
                cursor: *cursor,
                block: *source_block,
            }),
            _ => None,
        };
        Self {
            root,
            invocation,
            step,
            term: None,
        }
    }
}

/// The zone changes an occurrence reports, in member order.
fn movements(actual: &EventOutcome) -> Vec<&MoveOutcome> {
    match actual {
        EventOutcome::EnteredAttached { movement, .. }
        | EventOutcome::Moved { movement }
        | EventOutcome::Sacrificed { movement, .. }
        | EventOutcome::CardDiscarded { movement, .. }
        | EventOutcome::StateBasedMoved { movement, .. }
        | EventOutcome::LibraryTopMoved { movement, .. } => vec![movement],
        EventOutcome::StackObjectCountered { movement, .. } => movement.iter().collect(),
        EventOutcome::Milled { movements, .. } | EventOutcome::Surveilled { movements, .. } => {
            movements.iter().collect()
        }
        _ => vec![],
    }
}

impl RulesTxn {
    /// Read the settled balances exactly once, at the leaf's commit boundary.
    /// A zone change forgets old counters (CR 122.2), so only the entrant's
    /// initial counters count as changes. Reversal restores rows directly.
    pub(super) fn committed_counter_changes(
        &self,
        prior: &Self,
        id: &OccurrenceId,
        cause: &BatchCause,
        proposed: &ProposedEvent,
        outcome: &EventOutcome,
    ) -> Result<Vec<CounterChange>, String> {
        let putter = match cause {
            BatchCause::Instruction { player, .. }
            | BatchCause::AbilityActivation { player, .. }
            | BatchCause::SpecialAction { player, .. } => Some(*player),
            BatchCause::PrecombatMainLore => prior.state.rules.active,
            _ => None,
        };
        let mut changes = vec![];
        let mut permanent = |object, with_entry: Option<EventId>| -> Result<(), String> {
            let next = self.current(object)?;
            let previous = if with_entry.is_some() {
                None
            } else {
                Some(prior.current(object)?)
            };
            let holder = CounterHolder::Permanent {
                object,
                controller: previous.unwrap_or(next).controller,
            };
            let kinds: BTreeSet<_> = next
                .counter_kinds()
                .chain(previous.into_iter().flat_map(Object::counter_kinds))
                .collect();
            for counter in kinds {
                let before = previous.map_or_else(Quantity::zero, |o| o.counter_count(&counter));
                let current = next.counter_count(&counter);
                if before != current {
                    let row = if current > before {
                        next
                    } else {
                        previous.unwrap_or(next)
                    };
                    let timestamp = row
                        .counters
                        .iter()
                        .find(|stack| stack.kind == counter)
                        .map(|stack| stack.timestamp);
                    let placed_by = if current <= before {
                        None
                    } else if with_entry.is_some() {
                        // CR 122.6a: intrinsic entry counters are put on by
                        // the entering permanent's controller.
                        Some(next.controller)
                    } else {
                        putter
                    };
                    changes.push(CounterChange {
                        holder,
                        counter,
                        previous: before,
                        current,
                        placed_by,
                        timestamp,
                        with_entry: with_entry.clone(),
                    });
                }
            }
            Ok(())
        };
        match proposed {
            ProposedEvent::PutCounters { object, .. }
            | ProposedEvent::PayCounterCost { object, .. }
            | ProposedEvent::RemoveLoreCounter { object }
            | ProposedEvent::AnnihilateCounters { object, .. }
            | ProposedEvent::TurnFaceUp { object }
            | ProposedEvent::Untap { object } => permanent(*object, None)?,
            _ => {}
        }
        if let EventOutcome::DamageCommitted { actual, .. } = outcome {
            let recipients: BTreeSet<_> = actual
                .iter()
                .filter_map(|damage| match damage.recipient {
                    DamageRecipient::Permanent { object } => Some(object),
                    DamageRecipient::Player { .. } => None,
                })
                .collect();
            for object in recipients {
                permanent(object, None)?;
            }
        }

        let entry = &mut |member: usize, object| {
            permanent(
                object,
                Some(EventId {
                    occurrence: id.clone(),
                    member: u32::try_from(member).map_err(|_| "event member limit")?,
                }),
            )
        };
        match outcome {
            EventOutcome::TokenCopied { object, .. } => entry(0, *object)?,
            EventOutcome::TokensCreated { objects, .. }
            | EventOutcome::PredefinedTokensCreated { objects, .. }
            | EventOutcome::EncoreTokensCreated { objects, .. }
            | EventOutcome::MyriadTokensCreated { objects, .. } => {
                for (member, object) in objects.iter().enumerate() {
                    entry(member, *object)?;
                }
            }
            _ => {
                for (member, movement) in movements(outcome).into_iter().enumerate() {
                    if movement.from != Zone::Battlefield
                        && movement.destination == Zone::Battlefield
                    {
                        entry(member, movement.successor)?;
                    }
                }
            }
        }

        if let ProposedEvent::GivePlayerCounters { player, .. } = proposed {
            let previous = &prior.state.rules.players[player.0 as usize];
            let current = &self.state.rules.players[player.0 as usize];
            for (kind, before, after) in [
                (
                    "poison",
                    &previous.poison_counters,
                    &current.poison_counters,
                ),
                (
                    "energy",
                    &previous.energy_counters,
                    &current.energy_counters,
                ),
            ] {
                if before != after {
                    changes.push(CounterChange {
                        holder: CounterHolder::Player { player: *player },
                        counter: CounterKind::Named { name: kind.into() },
                        previous: before.clone(),
                        current: after.clone(),
                        placed_by: (after > before).then_some(putter).flatten(),
                        timestamp: None,
                        with_entry: None,
                    });
                }
            }
        }
        Ok(changes)
    }

    fn entered_facet(&self, object: ObjectId) -> Result<Facet, String> {
        let current = self.current(object)?;
        Ok(Facet::Entered {
            object,
            controller: current.controller,
            tapped: current.tapped,
            token: self.state.rules.tokens.contains_key(&object.card),
            attached_to: current.attached_to,
            face: current.face(),
        })
    }

    /// The single writer of creation, entry, movement and counter facts: the actual
    /// members of one committed occurrence, read against its prior.
    pub(crate) fn occurrence_actuals(
        &self,
        before: &RulesTxn,
        cause: &BatchCause,
        group: Option<&GroupId>,
        occurrence: &OccurrenceResult,
    ) -> Result<Vec<Actual>, String> {
        let mut origin = Origin::of(cause, before);
        if matches!(
            occurrence.proposed,
            ProposedEvent::PayCounterCost {
                cost: CounterCost::Loyalty { .. },
                ..
            }
        ) {
            origin.root = Root::Cost(CostOf::Loyalty);
        }
        let mut members: Vec<Vec<Facet>> = vec![];
        let created = |object: &ObjectId, creator: PlayerId| -> Result<Vec<Facet>, String> {
            let recipe = self
                .state
                .rules
                .tokens
                .get(&object.card)
                .ok_or("created token has no recipe")?
                .spec
                .clone();
            Ok(vec![
                Facet::Created {
                    creator,
                    recipe: Box::new(recipe),
                    invocation: origin.invocation,
                },
                self.entered_facet(*object)?,
            ])
        };
        match &occurrence.actual {
            EventOutcome::TokenCopied {
                object, controller, ..
            } => members.push(created(object, *controller)?),
            EventOutcome::TokensCreated {
                objects,
                controller,
                ..
            }
            | EventOutcome::PredefinedTokensCreated {
                objects,
                controller,
                ..
            }
            | EventOutcome::EncoreTokensCreated {
                objects,
                controller,
                ..
            }
            | EventOutcome::MyriadTokensCreated {
                objects,
                controller,
                ..
            } => {
                for object in objects {
                    members.push(created(object, *controller)?);
                }
            }
            EventOutcome::LeftGame { object, owner } => {
                let current = before.current(*object)?;
                members.push(vec![Facet::Moved {
                    object: *object,
                    prior: current
                        .present_on_battlefield()
                        .then(|| before.object_before(*object).map(Box::new))
                        .transpose()?,
                    from: current.zone,
                    to: Destination::LeftGame { owner: *owner },
                    successor: None,
                    how: MoveKind::Put,
                }]);
            }
            actual => {
                for m in movements(actual) {
                    let prior = before
                        .current(m.previous)
                        .is_ok_and(|o| o.present_on_battlefield())
                        .then(|| before.object_before(m.previous).map(Box::new))
                        .transpose()?;
                    let how = match cause {
                        BatchCause::SpellCompletion {
                            stack_id, source, ..
                        } if m.from == Zone::Stack && *source == m.previous => MoveKind::Resolved {
                            stack_id: *stack_id,
                        },
                        _ => MoveKind::Put,
                    };
                    let mut facets = vec![Facet::Moved {
                        object: m.previous,
                        prior,
                        from: m.from,
                        to: Destination::Zone(m.destination),
                        successor: Some(m.successor),
                        how,
                    }];
                    // CR 603.6a: a move onto the battlefield is an entry and
                    // never a creation, so a resolving permanent-spell copy
                    // enters as a token without a creation facet (CR 111.13).
                    if m.from != Zone::Battlefield && m.destination == Zone::Battlefield {
                        facets.push(self.entered_facet(m.successor)?);
                    }
                    members.push(facets);
                }
            }
        }
        let group = group.cloned().or_else(|| {
            (!occurrence.counter_changes.is_empty()
                && members.len() + occurrence.counter_changes.len() > 1)
                .then(|| GroupId {
                    batch: occurrence.id.batch,
                    path: occurrence.id.path.clone(),
                })
        });
        members.extend(
            occurrence
                .counter_changes
                .iter()
                .cloned()
                .map(|change| vec![Facet::Counters(change)]),
        );
        members
            .into_iter()
            .enumerate()
            .map(|(member, facets)| {
                Ok(Actual {
                    id: EventId {
                        occurrence: occurrence.id.clone(),
                        member: u32::try_from(member).map_err(|_| "event member limit")?,
                    },
                    group: group.clone(),
                    cause: cause.clone(),
                    origin,
                    lineage: None,
                    facets,
                })
            })
            .collect()
    }

    /// The single history reducer: the ledger change these actuals make,
    /// against the ledger before them. Entries count by the resulting
    /// controller and types (CR 603.6a), departures by the prior row (CR
    /// 603.10a, 700.4) and creations by the creating player (CR 111.2).
    pub(crate) fn reduce_history<'a>(
        &self,
        actuals: impl IntoIterator<Item = &'a Actual>,
    ) -> Result<LedgerDelta, String> {
        let mut delta = LedgerDelta::default();
        let history = &self.state.rules.cast_history;
        for actual in actuals {
            if let Some(entrant) = actual.entrant() {
                let controller = self.current(entrant)?.controller;
                let types = self.characteristics(entrant)?;
                let row = delta.seat(controller);
                if types.has_type(CardKind::Artifact) {
                    LedgerDelta::count(&mut row.artifacts_entered)?;
                }
                if types.has_type(CardKind::Creature) {
                    LedgerDelta::count(&mut row.creatures_entered)?;
                }
                if !types.has_type(CardKind::Land) {
                    LedgerDelta::count(&mut row.nonlands_entered)?;
                }
            }
            if let Some((prior, to_graveyard)) = actual.departure() {
                let was = history
                    .get(prior.controller.0 as usize)
                    .ok_or("invalid turn history seat")?;
                let row = delta.seat(prior.controller);
                row.permanent_left |= !was.permanent_left;
                row.creature_died |= to_graveyard
                    && !was.creature_died
                    && prior.characteristics.has_type(CardKind::Creature);
            }
            if let Some(Facet::Created { creator, .. }) = actual.facet(FacetKind::Created) {
                LedgerDelta::count(&mut delta.seat(*creator).tokens_created)?;
            }
        }
        delta.seats.retain(|row| !row.is_empty());
        Ok(delta)
    }

    /// The one publication of committed occurrences, in fixed order:
    /// last-known facts, history, control, triggers and watchers, then the
    /// step results. Every step runs whether or not anything matches, and
    /// none reads the trace.
    pub(super) fn publish_actuals(
        &mut self,
        before: &RulesTxn,
        cause: &BatchCause,
        group: Option<GroupId>,
        occurrences: &[OccurrenceResult],
        reconcile: bool,
    ) -> Result<(), String> {
        let actuals = occurrences
            .iter()
            .map(|o| self.occurrence_actuals(before, cause, group.as_ref(), o))
            .collect::<Result<Vec<_>, _>>()?;
        // 1. CR 603.10a, 702.15c/702.2e: simultaneous departures keep the
        // common prior layer-6 view, even if a granting source happened to
        // be applied first by deterministic event order.
        if group.is_some() {
            for o in before
                .state
                .rules
                .objects
                .values()
                .filter(|o| o.zone == Zone::Battlefield)
            {
                if self.current(o.id).is_err() {
                    self.retain_source_facts(&before.object_before(o.id)?);
                }
            }
        }
        // 2. One reducer over every member against the pre-group ledger.
        let delta = self.reduce_history(actuals.iter().flatten())?;
        if !delta.is_empty() {
            delta.apply(&mut self.rules_mut().cast_history)?;
        }
        self.record_unmigrated_history(occurrences)?;
        // 3. CR 110.2, 800.4a: control follows the committed group.
        if reconcile {
            self.reconcile_controllers()?;
        }
        // 4. Trigger matching and delayed watchers read the facets.
        self.collect_triggers(cause, before, occurrences, &actuals)?;
        // 5. Bounded procedures and the resolving counter step bind the
        // members from this publication.
        if let Some(journal) = self.actual_journal.as_deref_mut() {
            for (occurrence, members) in occurrences.iter().zip(&actuals) {
                if members.is_empty() {
                    journal.events.push(EventId {
                        occurrence: occurrence.id.clone(),
                        member: 0,
                    });
                }
                journal.events.extend(members.iter().map(|a| a.id.clone()));
                journal.actuals.extend(members.iter().cloned());
            }
            journal.ledger.merge(&delta)?;
        }
        if matches!(&self.state.rules.frame, Frame::CounterPlacement { transaction }
            if &transaction.cause == cause)
        {
            self.finish_counter_batch(actuals.iter().flatten())?;
        }
        Ok(())
    }
}

impl RulesTxn {
    /// Run `f` with a journal open, returning what it published. An
    /// enclosing journal also receives everything.
    pub(super) fn with_actual_journal<T>(
        &mut self,
        f: impl FnOnce(&mut Self) -> Result<T, String>,
    ) -> Result<(T, ActualJournal), String> {
        let outer = self.actual_journal.replace(Box::default());
        let result = f(self);
        let inner = std::mem::replace(&mut self.actual_journal, outer).unwrap_or_default();
        let result = result?;
        if let Some(outer) = self.actual_journal.as_deref_mut() {
            outer.events.extend(inner.events.iter().cloned());
            outer.actuals.extend(inner.actuals.iter().cloned());
            outer.ledger.merge(&inner.ledger)?;
        }
        Ok((result, *inner))
    }
}
