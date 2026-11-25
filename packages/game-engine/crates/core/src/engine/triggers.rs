use super::*;
use crate::query::{self, ObjectFacts, Tri};
use mtg_contracts::kir::{self, Atom, EventPattern, PlayerExpr, Pred, Ref};
use std::borrow::Cow;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TriggerId(pub u64);
pub fn is_zero(value: &u64) -> bool {
    *value == 0
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PendingTrigger {
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
    order: Vec<TriggerId>,
}
/// Batch occurrences and completed casts have independent identity domains.
/// The untagged batch alternative preserves the existing {batch,path} bytes;
/// cast IDs name the real SpellCast event, not an invented event-group slot.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TriggerOccurrenceId {
    Batch(OccurrenceId),
    Cast(CastOccurrenceId),
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CastOccurrenceId {
    pub cast: StackId,
}
/// Semantic successful-cast ledger, independent of observers and tracing.
/// Original physical seats never compact on departure (CR 601.2i/800.4).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TurnCastHistory {
    pub any_cast: bool,
    pub exile_cast: bool,
    pub mana_value: Quantity,
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
    }
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
        defender_player_id: PlayerId,
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
    StepBegan {
        turn: TurnState,
        owner: PlayerId,
        active: Option<PlayerId>,
    },
}
impl TriggerEventWitness {
    pub fn subject(&self) -> Option<ObjectId> {
        match self {
            Self::Entered { entrant, .. } => Some(entrant.object),
            Self::Moved { movement: m } => Some(if m.destination == Zone::Battlefield {
                m.successor
            } else {
                m.previous
            }),
            Self::TokenEntered { object } | Self::Tapped { object, .. } => Some(*object),
            Self::CreatureAttacked { attacker, .. } => Some(attacker.object),
            Self::DamageDealt { damage, .. } => Some(damage.source.object),
            Self::StepBegan { .. } | Self::SpellCast { .. } => None,
        }
    }
    pub(super) fn event_player(&self) -> Option<PlayerId> {
        match self {
            Self::StepBegan { active, .. } => *active,
            Self::SpellCast { cast } => Some(cast.caster),
            _ => None,
        }
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
            (EventPattern::Attacks { .. }, Self::CreatureAttacked { .. }) => Some(second),
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
    })
}

fn movement(outcome: &EventOutcome) -> Option<&MoveOutcome> {
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
            let witnesses = match &occurrence.actual {
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
                            defender_player_id: *defender,
                        });
                    }
                    witnesses
                }
                EventOutcome::Tapped { object }
                | EventOutcome::Regenerated {
                    object,
                    tapped: true,
                    ..
                } => self.source_tap_witness(*object)?.into_iter().collect(),
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
            for witness in witnesses {
                if witness.dies()
                    || witness.enters()
                    || matches!(
                        witness,
                        TriggerEventWitness::StepBegan { .. }
                            | TriggerEventWitness::CreatureAttacked { .. }
                            | TriggerEventWitness::DamageDealt { .. }
                            | TriggerEventWitness::Tapped { .. }
                    )
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
        }
        let dies = witness.dies();
        let enters = witness.enters();

        let subject = witness.subject();
        let observers = if dies {
            before
                .ok_or("death trigger requires actual pre-event lookback")?
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
        let dying = if dies {
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
            .filter(|o| !matches!(&witness, TriggerEventWitness::Tapped { object, .. } if o.id != *object))
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
                        if pred.reads_characteristics() {
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
                    EventPattern::Dies { object: pred } => dying.is_some_and(|dying| {
                        holds(pred, Some(&observers.object_facts(dying)), you, dying.id, &identity)
                    }),
                    EventPattern::BecomesTapped { object: pred } => {
                        matches!(&witness, TriggerEventWitness::Tapped { object: tapped, .. }
                            if observers.current(*tapped).is_ok_and(|t| {
                                holds(pred, Some(&observers.object_facts(t)), you, *tapped, &identity)
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
                    EventPattern::Cast { caster, spell } => {
                        matches!(&witness, TriggerEventWitness::SpellCast { cast }
                            if player_in(caster, Some(cast.caster), you)
                                && holds(spell, Some(&Frozen {
                                    facts: &cast.spell,
                                    token: false,
                                    live: observers.is_live(cast.caster),
                                }), you, cast.spell.object, &identity))
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
                    EventPattern::AnyOf { .. } => false,
                };
                if matches
                    && matches!((definition.functioning_zone, object.zone), (FunctioningZone::Battlefield, Zone::Battlefield) | (FunctioningZone::Stack, Zone::Stack))
                    && observers.is_live(object.controller)
                {
                    let mut matched_witness = entry_witness.unwrap_or_else(|| witness.clone());
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
                ));
                }
            }
        }

        // Preflight the COMPLETE effective emission before diagnostics, handles,
        // IDs or pending rows publish. Failed intervening checks still retain
        // their ordinary diagnostic below, but consume no trigger capacity.
        let emission = matched
            .iter()
            .filter(|(_, _, _, condition, _)| condition.as_ref().is_none_or(|c| c.passed()))
            .count();
        self.preflight_trigger_emission(emission)?;
        for (death_controller, source_facts, ability, match_condition, witness) in matched {
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
            self.check_stack_capacity(1)?;
            let id = TriggerId(self.state.rules.next_trigger_id);
            self.rules_mut().next_trigger_id = self
                .state
                .rules
                .next_trigger_id
                .checked_add(1)
                .ok_or("trigger ID resource limit")?;
            let source = source_facts.object;
            let controller = source_facts.controller;
            let consumed = source_facts.clone();
            let source_handles = self.capture_trigger_source_handles(source)?;

            self.rules_mut().pending_triggers.push(PendingTrigger {
                death_controller,
                match_condition,
                id,
                source,
                controller,
                ability,

                source_facts,
                source_handles,
                event: event.clone(),
                witness: witness.clone(),
            });
            self.record_event(Event::TriggerCollected {
                trigger_id: id,
                source,
                player: controller,
                ability,

                occurrence: event.clone(),
                witness: witness.clone(),
                source_facts: Box::new(consumed),
            })?;
        }
        self.check_history_capacity()?;
        Ok(())
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
            .filter(|t| t.controller == *player)
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
                    let targeted = !self
                        .effective_trigger(trigger.source, trigger.ability)?
                        .program
                        .targets
                        .is_empty();
                    let zero = ManaCost {
                        generic: Quantity::zero(),
                        colored: vec![],
                    };
                    self.rules_mut().stack.push(StackInstance {
                        mode_mask: None,
                        hybrid_colors: vec![],
                        cost_lock: None,
                        selected_additional_costs: None,

                        x: None,
                        death_controller: trigger.death_controller,
                        sacrificed: None,
                        sacrificed_set: vec![],
                        effect_source_handles: vec![],
                        source_facts: Some(trigger.source_facts.clone()),
                        entity_targets: vec![],
                        announced_entity_targets: vec![],
                        stack_targets: vec![],
                        announced_stack_targets: vec![],
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
                    let top = self
                        .rules_mut()
                        .stack
                        .pop()
                        .ok_or("missing impossible targeted trigger")?;
                    let StackKind::Triggered { trigger_id, .. } = top.kind else {
                        return Err("impossible trigger kind".into());
                    };
                    self.record_event(Event::TriggeredAbilityNotPlaced {
                        trigger_id,
                        stack_id,
                        reason: TriggerPlacementFailure::NoLegalTargets,
                    })?;
                    self.rules_mut().frame = Frame::TriggerOrder { placement };
                    return Ok(());
                }
                if self.program()?.targets.first().is_some_and(|t| {
                    matches!(
                        t.domain,
                        TargetDomain::BattlefieldPermanent
                            | TargetDomain::ControlledBattlefieldPermanent
                            | TargetDomain::BattlefieldCreature
                            | TargetDomain::ControlledBattlefieldCreature
                            | TargetDomain::OwnGraveyardCreatureCard
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
        ability: EffectiveTriggerAbility,
        controller: PlayerId,
        fact: &ObjectBefore,
        handles: &[publish::StoredObjectHandle],
    ) -> Result<(), String> {
        let definition = self.effective_trigger(source, ability)?;
        if controller.0 >= 4
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
            || definition.functioning_zone != FunctioningZone::Battlefield
            || fact.zone != Zone::Battlefield
            || (*definition.condition.pattern() == EventPattern::self_enters()
                && fact.controller != controller)
            || handles.len() != 4
        {
            return Err("snapshot trigger source invariant".into());
        }
        {
            self.validate_characteristic_identity(source, &fact.characteristics)?;
        }
        Ok(())
    }
    fn valid_entry_subject(
        &self,
        entrant: &ObjectBefore,
        movement: Option<&MoveOutcome>,
    ) -> Result<bool, String> {
        self.validate_characteristic_identity(entrant.object, &entrant.characteristics)?;
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
        self.validate_characteristic_identity(cast.spell.object, &cast.spell.characteristics)?;
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
            && cast.x.as_ref().is_none_or(|x| !x.is_negative()))
    }

    fn validate_trigger_witness(
        &self,
        source: ObjectId,
        ability: EffectiveTriggerAbility,
        controller: PlayerId,
        death_controller: Option<PlayerId>,
        witness: &TriggerEventWitness,
    ) -> Result<(), String> {
        let definition = self.effective_trigger(source, ability)?;
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
            }) || self.copiable_definition(object)?.characteristics.kind == CardKind::Creature)
        };
        // CR 603.10a: exactly the reviewed opponent-death row freezes the
        // dying creature's controller, pending or on the stack alike.
        let binds = binds_death_controller(&definition);
        if death_controller.is_some() != binds
            || death_controller.is_some_and(|p| p.0 >= 4 || p == controller)
        {
            return Err("snapshot X/death LKI binding invariant".into());
        }
        let valid = match witness {
            TriggerEventWitness::DamageDealt {
                kind,
                transaction,
                damage,
                match_source,
            } => {
                let fact = &damage.source;
                self.validate_characteristic_identity(fact.object, &fact.characteristics)?;
                if let Some(appearance) = match_source {
                    self.validate_characteristic_identity(
                        appearance.object,
                        &appearance.characteristics,
                    )?;
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
            } => {
                self.validate_characteristic_identity(attacker.object, &attacker.characteristics)?;
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
            TriggerEventWitness::SpellCast { cast } => {
                self.valid_cast_facts(cast)?
                    && matches!(pattern, EventPattern::Cast { caster, spell }
                        if player_in(caster, Some(cast.caster), controller)
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
                        if !pred.reads_characteristics()
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
            TriggerEventWitness::Entered { entrant, movement } => {
                death_controller.is_none()
                    && self.valid_entry_subject(entrant, movement.as_ref())?
                    && entrant.zone == Zone::Battlefield
                    && matches!(pattern, EventPattern::Enters { object: pred }
                        if pred.reads_characteristics()
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
                        // An entry whose pattern read characteristics
                        // retains them as `Entered`.
                        EventPattern::Enters { object: pred } => {
                            witness.enters()
                                && !pred.reads_characteristics()
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
            (TriggerOccurrenceId::Batch(id), w) => {
                !matches!(w, TriggerEventWitness::SpellCast { .. })
                    && id.batch < self.state.rules.next_batch_id
            }
            _ => false,
        }
    }

    fn validate_alternative_occurrence<'a>(
        &self,
        source: ObjectId,
        ability: EffectiveTriggerAbility,
        event: &'a TriggerOccurrenceId,
        seen: &mut BTreeSet<(ObjectId, EffectiveTriggerAbility, u64, &'a [u32])>,
    ) -> Result<(), String> {
        if matches!(
            self.effective_trigger(source, ability)?.condition.pattern(),
            EventPattern::AnyOf { .. }
        ) {
            let TriggerOccurrenceId::Batch(occurrence) = event else {
                return Err("snapshot mixed trigger has no batch occurrence".into());
            };
            if !seen.insert((source, ability, occurrence.batch, &occurrence.path)) {
                return Err("snapshot duplicate mixed trigger occurrence".into());
            }
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
            *ability,
            entry.controller,
            match_condition.as_ref(),
            witness,
        )?;
        self.validate_trigger_witness(
            entry.source,
            *ability,
            entry.controller,
            entry.death_controller,
            witness,
        )?;
        if trigger_id.0 >= self.state.rules.next_trigger_id
            || !self.valid_trigger_occurrence(event, witness)
            || entry.cast_from != Zone::Battlefield
            || (!entry.targets.is_empty()
                && self.program_for(entry)?.targets.first().is_none_or(|t| {
                    !matches!(
                        t.domain,
                        TargetDomain::BattlefieldPermanent
                            | TargetDomain::BattlefieldCreature
                            | TargetDomain::OwnGraveyardCreatureCard
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
        let mut alternatives = BTreeSet::new();
        for trigger in &self.state.rules.pending_triggers {
            self.validate_trigger_source(
                trigger.source,
                trigger.ability,
                trigger.controller,
                &trigger.source_facts,
                &trigger.source_handles,
            )?;
            self.validate_match_condition(
                trigger.source,
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
            self.validate_trigger_witness(
                trigger.source,
                trigger.ability,
                trigger.controller,
                trigger.death_controller,
                &trigger.witness,
            )?;
            self.validate_alternative_occurrence(
                trigger.source,
                trigger.ability,
                &trigger.event,
                &mut alternatives,
            )?;
        }
        for entry in &self.state.rules.stack {
            if let Some(id) = self.validate_trigger_entry(entry)?
                && !ids.insert(id)
            {
                return Err("snapshot duplicate trigger identity".into());
            }
            if let StackKind::Triggered { ability, event, .. } = &entry.kind {
                self.validate_alternative_occurrence(
                    entry.source,
                    *ability,
                    event,
                    &mut alternatives,
                )?;
            }
        }
        if let Frame::TriggerOrder { placement } | Frame::TriggerTargets { placement, .. } =
            &self.state.rules.frame
        {
            if placement.remaining.iter().any(|p| !self.is_live(*p))
                || placement.remaining.iter().collect::<BTreeSet<_>>().len()
                    != placement.remaining.len()
                || placement.order.iter().collect::<BTreeSet<_>>().len() != placement.order.len()
                || placement.order.iter().any(|id| {
                    !self
                        .state
                        .rules
                        .pending_triggers
                        .iter()
                        .any(|t| t.id == *id && Some(&t.controller) == placement.remaining.first())
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
        self.rules_mut().frame = Frame::TriggerOrder { placement };
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "origin", rename_all = "snake_case", deny_unknown_fields)]
pub enum EffectiveTriggerAbility {
    Intrinsic { index: u32 },
}

impl RulesTxn {
    pub(super) fn effective_trigger(
        &self,
        source: ObjectId,
        ability: EffectiveTriggerAbility,
    ) -> Result<std::borrow::Cow<'_, mtg_contracts::TriggeredAbility>, String> {
        match ability {
            EffectiveTriggerAbility::Intrinsic { index } => self
                .copiable_definition(source)?
                .triggered
                .get(index as usize)
                .map(std::borrow::Cow::Borrowed)
                .ok_or_else(|| "missing intrinsic triggered definition".into()),
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
        let mut charge = 0usize;
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
