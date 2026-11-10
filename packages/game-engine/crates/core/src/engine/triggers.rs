use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TriggerId(pub u64);
pub(super) fn is_zero(value: &u64) -> bool {
    *value == 0
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PendingTrigger {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    death_controller: Option<PlayerId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) match_condition: Option<InterveningCheck>,
    pub(super) id: TriggerId,
    pub(super) source: ObjectId,
    pub(super) controller: PlayerId,
    pub(super) ability: u32,
    pub(super) source_facts: ObjectBefore,
    pub(super) source_handles: Vec<ObjectHandle>,
    event: TriggerOccurrenceId,
    witness: TriggerEventWitness,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TriggerPlacement {
    resume: Box<Frame>,
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
/// Frozen CR 601.2i event facts. The caster is not the active player.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CastOccurrence {
    pub stack_id: StackId,
    pub caster: PlayerId,
    pub cast_from: Zone,
    pub spell: ObjectBefore,
}
/// Actual trigger-event evidence, independent of whether the event has an object.
/// Source/controller facts are frozen separately at collection (CR 603.3a).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case", deny_unknown_fields)]
pub enum TriggerEventWitness {
    /// Immutable damage-time facts, independent of departure-refreshed source
    /// LKI. None identifies the genuine legacy CombatDamage leaf; Some names
    /// the original frozen transaction, never the current combat state.
    DamageDealt {
        kind: DamageKind,
        transaction: Option<DamageTransactionId>,
        damage: Box<ActualDamage>,
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
    StepBegan {
        turn: TurnState,
        owner: PlayerId,
        active: Option<PlayerId>,
    },
}
impl TriggerEventWitness {
    pub fn subject(&self) -> Option<ObjectId> {
        match self {
            Self::Moved { movement: m } => Some(if m.destination == Zone::Battlefield {
                m.successor
            } else {
                m.previous
            }),
            Self::TokenEntered { object } => Some(*object),
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
        matches!(self, Self::TokenEntered { .. })
            || matches!(self, Self::Moved { movement: m } if m.destination == Zone::Battlefield && m.from != Zone::Battlefield)
    }
}
// CR 109.5/603.3a/603.10: an ordinary trigger observes its source's
// controller at the occurrence, independently of the controller of the damage
// invocation and of assignment-time source facts. Restore uses that frozen
// trigger controller, never a later controller or current recipient liveness.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
pub(super) fn damage_matches(
    condition: &TriggerCondition,
    source: ObjectId,
    controller: PlayerId,
    kind: DamageKind,
    damage: &ActualDamage,
) -> bool {
    let TriggerCondition::DamageDealt {
        source: subject,
        kind: required_kind,
        recipient,
    } = condition
    else {
        return false;
    };
    let subject_matches = match subject {
        DamageTriggerSource::Source {} => damage.source.object == source,
        DamageTriggerSource::CreatureFilter {
            controller: relation,
            types,
        } => {
            // CR 109.5/205.2b/603.2c: a creature domain and the complete type
            // predicate both match the actual damager's frozen characteristics.
            // Its damage-time controller differs from its owner and from the
            // controller of a noncombat invocation that made it deal damage.
            damage.source.zone == Zone::Battlefield
                && damage.source.characteristics.has_type(CardKind::Creature)
                && types.matches(&damage.source.characteristics)
                && match relation {
                    DamageSourceRelation::Controller => damage.source.controller == controller,
                }
        }
    };
    subject_matches
        && damage.amount > Quantity::zero()
        && matches!(
            (required_kind, kind),
            (TriggerDamageKind::Any, _) | (TriggerDamageKind::Combat, DamageKind::Combat)
        )
        && matches!(damage.recipient, DamageRecipient::Player { player }
        if match recipient {
            DamageTriggerRecipient::Player => true,
            DamageTriggerRecipient::OpponentPlayer => player != controller,
        })
}
fn step_matches(step: BeginningStep, turn: &TurnState) -> bool {
    matches!(
        (step, turn.step),
        (BeginningStep::Upkeep, TurnStep::Upkeep)
            | (BeginningStep::Draw, TurnStep::Draw)
            | (BeginningStep::End, TurnStep::End)
    )
}
fn relation_matches(
    relation: TurnRelation,
    active: Option<PlayerId>,
    controller: PlayerId,
) -> bool {
    // CR 800.4j: the turn continues without an active player. Unqualified
    // "each upkeep" still matches; "your" and "opponent's" need that player.
    match relation {
        TurnRelation::AnyPlayer => true,
        TurnRelation::Controller => active == Some(controller),
        TurnRelation::Opponent => active.is_some_and(|p| p != controller),
    }
}
fn caster_matches(relation: CasterRelation, caster: PlayerId, controller: PlayerId) -> bool {
    match relation {
        CasterRelation::AnyPlayer => true,
        CasterRelation::Controller => caster == controller,
        CasterRelation::Opponent => caster != controller,
    }
}
fn attack_matches(
    predicate: &AttackSubject,
    source: ObjectId,
    controller: PlayerId,
    attacker: &ObjectBefore,
) -> bool {
    if !attacker.characteristics.has_type(CardKind::Creature) {
        return false;
    }
    match predicate {
        AttackSubject::Source {} => attacker.object == source,
        AttackSubject::Filter {
            controller: relation,
            types,
        } => {
            (match relation {
                AttackerRelation::AnyPlayer => true,
                AttackerRelation::Controller => attacker.controller == controller,
                AttackerRelation::Opponent => attacker.controller != controller,
            }) && types.matches(&attacker.characteristics)
        }
    }
}
fn movement(outcome: &EventOutcome) -> Option<&MoveOutcome> {
    match outcome {
        EventOutcome::Moved { movement }
        | EventOutcome::Sacrificed { movement, .. }
        | EventOutcome::CardDiscarded { movement, .. }
        | EventOutcome::StateBasedMoved { movement, .. }
        | EventOutcome::LibraryTopMoved { movement, .. } => Some(movement),
        EventOutcome::StackObjectCountered { movement, .. } => movement.as_ref(),
        _ => None,
    }
}
impl Game {
    pub(super) fn stack_limit(&self) -> usize {
        capacities::STACK_LIMIT
    }
    // CR 603.2c/603.3a/603.10a: one match per actual occurrence, with
    // pre-event lookback for dies and post-event observers for enters. An
    // atomic simultaneous group shares one observer view, including sources
    // that die together. LeaveGame/exile/replaced command moves are not dies.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    pub(super) fn collect_triggers(
        &mut self,
        before: &Game,
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
                EventOutcome::AttackersDeclared { attacks } => attacks
                    .iter()
                    .map(|(attacker, defender)| {
                        Ok(TriggerEventWitness::CreatureAttacked {
                            attacker: Box::new(before.object_before(*attacker)?),
                            defender_player_id: *defender,
                        })
                    })
                    .collect::<Result<Vec<_>, String>>()?,
                EventOutcome::TokensCreated { objects, .. } => objects
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
                    )
                {
                    self.collect_trigger_witness(
                        Some(before),
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
    pub(super) fn collect_cast_triggers(&mut self, entry: &StackInstance) -> Result<(), String> {
        let completed = self.top()?;
        if completed.id != entry.id || !completed.kind.is_spell() || completed.paid.is_none() {
            return Err("cast occurrence requires a completed paid spell".into());
        }
        let witness = TriggerEventWitness::SpellCast {
            cast: Box::new(CastOccurrence {
                stack_id: entry.id,
                caster: entry.controller,
                cast_from: entry.cast_from,
                spell: self.object_before(entry.source)?,
            }),
        };
        self.collect_trigger_witness(
            None,
            TriggerOccurrenceId::Cast(CastOccurrenceId { cast: entry.id }),
            witness,
        )
    }
    fn collect_trigger_witness(
        &mut self,
        before: Option<&Game>,
        event: TriggerOccurrenceId,
        witness: TriggerEventWitness,
    ) -> Result<(), String> {
        let dies = witness.dies();
        let enters = witness.enters();

        let subject = witness.subject();
        let creature = subject.is_some_and(|o| {
            self.definition(o.card)
                .is_ok_and(|d| d.characteristics.kind == CardKind::Creature)
        });
        let observers = if dies {
            before.ok_or("death trigger requires actual pre-event lookback")?
        } else {
            &*self
        };
        let mut matched = vec![];
        for object in observers
            .state
            .rules
            .objects
            .values()
            .filter(|o| o.zone == Zone::Battlefield)
        {
            for (ability, definition) in observers
                .definition(object.id.card)?
                .triggered
                .iter()
                .enumerate()
            {
                let matches = match &definition.condition {
                    TriggerCondition::DamageDealt { .. } => {
                        matches!(&witness, TriggerEventWitness::DamageDealt { kind, damage, .. }
                            if damage_matches(&definition.condition, object.id, object.controller, *kind, damage))
                    }
                    TriggerCondition::CreatureAttacked {
                        attacker: predicate,
                    } => {
                        matches!(&witness, TriggerEventWitness::CreatureAttacked { attacker, .. }
                            if attack_matches(predicate, object.id, object.controller, attacker))
                    }
                    TriggerCondition::SpellCast { caster, spell } => {
                        matches!(&witness, TriggerEventWitness::SpellCast {cast} if caster_matches(*caster, cast.caster, object.controller) && spell.matches(&cast.spell.characteristics))
                    }
                    TriggerCondition::StepBegan {
                        step,
                        turn_relation,
                    } => {
                        matches!(&witness, TriggerEventWitness::StepBegan { turn, active, .. } if step_matches(*step, turn) && relation_matches(*turn_relation, *active, object.controller))
                    }
                    TriggerCondition::OpponentCreatureDies => {
                        creature
                            && dies
                            && observers
                                .current(subject.ok_or("missing object trigger subject")?)
                                .is_ok_and(|s| s.controller != object.controller)
                    }
                    TriggerCondition::SelfDies => creature && dies && Some(object.id) == subject,
                    TriggerCondition::SelfAuraToGraveyard => dies && Some(object.id) == subject,
                    TriggerCondition::AnyCreatureDies => creature && dies,
                    TriggerCondition::SelfEntersBattlefield => enters && Some(object.id) == subject,
                    TriggerCondition::OtherCreatureEntersBattlefield => {
                        creature && enters && Some(object.id) != subject
                    }
                };
                let matches = matches
                    && definition.death_filter.as_ref().is_none_or(|f| {
                        let Some(subject) = subject else {
                            return false;
                        };
                        let Ok(dying) = observers.current(subject) else {
                            return false;
                        };
                        let Ok(d) = observers.definition(subject.card) else {
                            return false;
                        };
                        (!f.controlled || dying.controller == object.controller)
                            && (!f.other || subject != object.id)
                            && (!f.non_token
                                || !observers.state.rules.tokens.contains_key(&subject.card))
                            && !d
                                .characteristics
                                .subtypes
                                .iter()
                                .any(|s| f.non_subtypes.contains(s))
                    });
                if matches
                    && definition.functioning_zone == FunctioningZone::Battlefield
                    && observers.is_live(object.controller)
                {
                    matched.push((
                        if definition.condition == TriggerCondition::OpponentCreatureDies {
                            Some(
                                observers
                                    .current(subject.ok_or("missing object trigger subject")?)?
                                    .controller,
                            )
                        } else {
                            None
                        },
                        observers.object_before(object.id)?,
                        u32::try_from(ability).map_err(|_| "trigger ability index limit")?,
                        definition
                            .intervening_if
                            .as_ref()
                            .map(|condition| {
                                observers.check_intervening(
                                    condition,
                                    object.controller,
                                    object.id,
                                    &observers.object_before(object.id)?,
                                )
                            })
                            .transpose()?,
                    ));
                }
            }
        }
        for (death_controller, source_facts, ability, match_condition) in matched {
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
            self.state.rules.next_trigger_id = self
                .state
                .rules
                .next_trigger_id
                .checked_add(1)
                .ok_or("trigger ID resource limit")?;
            let source = source_facts.object;
            let controller = source_facts.controller;
            let consumed = source_facts.clone();
            let source_handles = self.capture_trigger_source_handles(source)?;
            self.state.rules.pending_triggers.push(PendingTrigger {
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
        Ok(())
    }
    fn apnap_players(&self) -> Vec<PlayerId> {
        let anchor = self.state.rules.turn_owner.0;
        (0..4)
            .map(|offset| PlayerId((anchor + offset) % 4))
            .filter(|p| self.is_live(*p))
            .collect()
    }
    pub(super) fn begin_trigger_placement(
        &mut self,
        resume: Frame,
        cleanup: bool,
        changed: bool,
    ) -> Result<bool, String> {
        if self.state.rules.pending_triggers.is_empty() {
            return Ok(false);
        }
        let live = self.live_players();
        self.state
            .rules
            .pending_triggers
            .retain(|t| live.contains(&t.controller));
        if self.state.rules.pending_triggers.is_empty() {
            return Ok(false);
        }
        self.state.rules.frame = Frame::TriggerOrder {
            placement: TriggerPlacement {
                resume: Box::new(resume),
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
    pub(super) fn trigger_order_ids(&self) -> Vec<TriggerId> {
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
    pub(super) fn trigger_request(&self) -> Result<Option<Request>, String> {
        let Frame::TriggerOrder { placement } = &self.state.rules.frame else {
            return Ok(None);
        };
        let candidates = self.trigger_order_ids();
        if candidates.len() > 1 {
            Ok(Some(Request::OrderTriggers {
                decision_id: self.state.rules.revision,
                player: *placement
                    .remaining
                    .first()
                    .ok_or("missing trigger controller")?,
                candidates,
            }))
        } else {
            Ok(None)
        }
    }
    pub(super) fn validate_trigger_response(
        &self,
        player: PlayerId,
        response: &Response,
    ) -> Option<Result<(), String>> {
        let (Frame::TriggerOrder { .. }, Response::OrderTriggers { order }) =
            (&self.state.rules.frame, response)
        else {
            return None;
        };
        Some((|| {
            let Some(Request::OrderTriggers {
                player: expected,
                candidates,
                ..
            }) = self.trigger_request()?
            else {
                return Err("no trigger order decision".into());
            };
            if player != expected
                || order.len() != candidates.len()
                || order.iter().copied().collect::<BTreeSet<_>>()
                    != candidates.into_iter().collect()
            {
                return Err("invalid trigger ordering".into());
            }
            Ok(())
        })())
    }
    fn finish_trigger_placement(&mut self, placement: TriggerPlacement) -> Result<(), String> {
        self.state.rules.frame = Frame::Stabilize {
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
    pub(super) fn trigger_step(&mut self) -> Result<(), String> {
        match self.state.rules.frame.clone() {
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
                    let trigger = self.state.rules.pending_triggers.remove(index);
                    self.validate_match_condition(
                        trigger.source,
                        trigger.ability,
                        trigger.controller,
                        trigger.match_condition.as_ref(),
                    )?;
                    if !self.is_live(trigger.controller) {
                        self.state.rules.frame = Frame::TriggerOrder { placement };
                        return Ok(());
                    }
                    let stack_id = StackId(self.state.rules.next_stack_id);
                    self.state.rules.next_stack_id = self
                        .state
                        .rules
                        .next_stack_id
                        .checked_add(1)
                        .ok_or("stack ID resource limit")?;
                    let targeted = !self
                        .definition(trigger.source.card)?
                        .triggered
                        .get(trigger.ability as usize)
                        .ok_or("missing trigger definition")?
                        .program
                        .targets
                        .is_empty();
                    let zero = ManaCost {
                        generic: Quantity::zero(),
                        colored: vec![],
                    };
                    self.state.rules.stack.push(StackInstance {
                        x: None,
                        death_controller: trigger.death_controller,
                        sacrificed: None,
                        mode: None,
                        effect_source_handles: vec![],
                        source_facts: Some(trigger.source_facts),
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
                        cast_from: Zone::Battlefield,
                    });
                    placement.changed |= placement.cleanup;
                    self.state.rules.passes.clear();
                    if targeted {
                        self.state.rules.frame = Frame::TriggerTargets {
                            placement,
                            stack_id,
                        };
                    } else {
                        self.record_event(Event::TriggerPlaced {
                            trigger_id: id,
                            stack_id,
                            player: trigger.controller,
                        })?;
                        self.state.rules.frame = Frame::TriggerOrder { placement };
                    }
                } else if self.trigger_request()?.is_some() {
                    let input = self
                        .state
                        .rules
                        .pending
                        .take()
                        .ok_or("missing trigger ordering input")?;
                    self.validate_trigger_response(input.player, &input.response)
                        .ok_or("trigger order response mismatch")??;
                    let Response::OrderTriggers { order } = input.response else {
                        return Err("trigger order response invariant".into());
                    };
                    self.record_event(Event::TriggersOrdered {
                        player: input.player,
                        order: order.clone(),
                    })?;
                    placement.order = order;
                    self.state.rules.frame = Frame::TriggerOrder { placement };
                } else if placement.remaining.is_empty() {
                    self.finish_trigger_placement(placement)?;
                } else {
                    let candidates = self.group_ids(&placement);
                    if candidates.is_empty() {
                        placement.remaining.remove(0);
                    } else {
                        placement.order = candidates;
                    }
                    self.state.rules.frame = Frame::TriggerOrder { placement };
                }
            }
            Frame::TriggerTargets {
                placement,
                stack_id,
            } => {
                if !self.has_target_domain(self.program()?, self.top()?.controller) {
                    self.state.rules.pending = None;
                    let top = self
                        .state
                        .rules
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
                    self.state.rules.frame = Frame::TriggerOrder { placement };
                    return Ok(());
                }
                if self.program()?.targets.first().is_some_and(|t| {
                    matches!(
                        t.domain,
                        TargetDomain::BattlefieldPermanent
                            | TargetDomain::BattlefieldCreature
                            | TargetDomain::OwnGraveyardCreatureCard
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
                    .state
                    .rules
                    .pending
                    .take()
                    .ok_or("missing trigger target input")?;
                let Response::PlayerTargets { players } = input.response else {
                    return Err("trigger target response invariant".into());
                };
                self.validate_actions_response(
                    input.player,
                    &Response::PlayerTargets {
                        players: players.clone(),
                    },
                )
                .ok_or("missing trigger target validation")??;
                let top = self
                    .state
                    .rules
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
                self.state.rules.frame = Frame::TriggerOrder { placement };
            }
            _ => return Err("trigger placement frame invariant".into()),
        }
        Ok(())
    }
    pub(super) fn revalidate_trigger_departure(&mut self) -> Result<(), String> {
        let live = self.live_players();
        self.state
            .rules
            .pending_triggers
            .retain(|t| live.contains(&t.controller));
        let revalidate = |mut placement: TriggerPlacement, game: &Game| {
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
        match self.state.rules.frame.clone() {
            Frame::TriggerOrder { placement } => {
                self.state.rules.frame = Frame::TriggerOrder {
                    placement: revalidate(placement, self),
                };
                if self
                    .state
                    .rules
                    .pending
                    .as_ref()
                    .is_some_and(|p| self.validate_response(p.player, &p.response).is_err())
                {
                    self.state.rules.pending = None;
                }
            }
            Frame::TriggerTargets {
                placement,
                stack_id,
            } => {
                let placement = revalidate(placement, self);
                if !self.state.rules.stack.iter().any(|s| s.id == stack_id) {
                    self.state.rules.pending = None;
                    self.state.rules.frame = Frame::TriggerOrder { placement };
                } else {
                    self.state.rules.frame = Frame::TriggerTargets {
                        placement,
                        stack_id,
                    };
                    if self
                        .state
                        .rules
                        .pending
                        .as_ref()
                        .is_some_and(|p| self.validate_response(p.player, &p.response).is_err())
                    {
                        self.state.rules.pending = None;
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
        ability: u32,
        controller: PlayerId,
        fact: &ObjectBefore,
        handles: &[ObjectHandle],
    ) -> Result<(), String> {
        let definition = self
            .definition(source.card)?
            .triggered
            .get(ability as usize)
            .ok_or("snapshot missing triggered definition")?;
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
            || fact.zone != Zone::Battlefield
            || handles.len() != 4
            || definition.functioning_zone != FunctioningZone::Battlefield
        {
            return Err("snapshot trigger source invariant".into());
        }
        {
            self.validate_characteristic_identity(source, &fact.characteristics)?;
        }
        Ok(())
    }
    fn validate_trigger_witness(
        &self,
        source: ObjectId,
        ability: u32,
        controller: PlayerId,
        death_controller: Option<PlayerId>,
        witness: &TriggerEventWitness,
    ) -> Result<(), String> {
        let condition = &self.definition(source.card)?.triggered[ability as usize].condition;
        let valid = match witness {
            TriggerEventWitness::DamageDealt {
                kind,
                transaction,
                damage,
            } => {
                let fact = &damage.source;
                self.validate_characteristic_identity(fact.object, &fact.characteristics)?;
                transaction.is_none_or(|id| id.0 < self.state.rules.next_damage_transaction_id)
                    && (damage.id.0 as usize) < self.damage_limit()
                    && damage.amount > Quantity::zero()
                    && fact.zone == Zone::Battlefield
                    && fact.characteristics.has_type(CardKind::Creature)
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
                    && damage_matches(condition, source, controller, *kind, damage)
                    && death_controller.is_none()
                // No live source/recipient or current combat requirement. CR
                // 113.7a/800.4 preserve a surviving controller's invocation.
                // Accepted-input replay proves actual member/kind/final amount,
                // including coherent rehashed lies that satisfy these guards.
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
                    && matches!(condition, TriggerCondition::CreatureAttacked { attacker: predicate }
                        if attack_matches(predicate, source, controller, attacker))
                    && death_controller.is_none()
                // Deliberately no current combat/player-liveness requirement:
                // CR 113.7a/800.4 preserve a surviving controller's invocation.
                // Accepted-input replay proves the actual declaration/facts.
            }
            TriggerEventWitness::SpellCast { cast } => {
                self.validate_characteristic_identity(
                    cast.spell.object,
                    &cast.spell.characteristics,
                )?;
                cast.stack_id.0 < self.state.rules.next_stack_id
                    && cast.caster.0 < 4
                    && cast.spell.controller == cast.caster
                    && cast.spell.default_controller.0 < 4
                    && cast.spell.zone == Zone::Stack
                    && !cast.spell.characteristics.has_type(CardKind::Land)
                    && self
                        .state
                        .rules
                        .cards
                        .get(&cast.spell.object.card)
                        .is_some_and(|c| c.owner == cast.spell.owner)
                    && matches!(cast.cast_from, Zone::Hand { owner } | Zone::Command { owner } if owner == cast.spell.owner)
                    && matches!(condition, TriggerCondition::SpellCast {caster,spell} if caster_matches(*caster, cast.caster, controller) && spell.matches(&cast.spell.characteristics))
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
                    && matches!(condition, TriggerCondition::StepBegan { step, turn_relation } if step_matches(*step, turn) && relation_matches(*turn_relation, *active, controller))
                    && death_controller.is_none()
            }
            TriggerEventWitness::TokenEntered { object } => {
                self.state.rules.tokens.contains_key(&object.card)
                    && object.incarnation == 0
                    && *condition == TriggerCondition::OtherCreatureEntersBattlefield
                    && *object != source
            }
            TriggerEventWitness::Moved { movement: m } => {
                self.state.rules.cards.contains_key(&m.previous.card)
                    && m.successor.card == m.previous.card
                    && m.previous.incarnation.checked_add(1) == Some(m.successor.incarnation)
                    && (match condition {
                        TriggerCondition::StepBegan { .. }
                        | TriggerCondition::SpellCast { .. }
                        | TriggerCondition::CreatureAttacked { .. }
                        | TriggerCondition::DamageDealt { .. } => false,
                        TriggerCondition::SelfDies | TriggerCondition::SelfAuraToGraveyard => {
                            witness.dies() && m.previous == source
                        }
                        TriggerCondition::OpponentCreatureDies => {
                            witness.dies()
                                && death_controller.is_some_and(|p| p.0 < 4 && p != controller)
                        }
                        TriggerCondition::AnyCreatureDies => witness.dies(),
                        TriggerCondition::SelfEntersBattlefield => {
                            witness.enters() && m.successor == source
                        }
                        TriggerCondition::OtherCreatureEntersBattlefield => {
                            witness.enters() && m.successor != source
                        }
                    })
                    && (matches!(
                        condition,
                        TriggerCondition::SelfAuraToGraveyard
                            | TriggerCondition::SelfEntersBattlefield
                    ) || self.definition(m.previous.card)?.characteristics.kind
                        == CardKind::Creature)
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
    pub(super) fn validate_trigger_entry(
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
    pub(super) fn validate_trigger_state(&self) -> Result<(), String> {
        if self.state.rules.pending_triggers.len() + self.state.rules.stack.len()
            > self.stack_limit()
        {
            return Err("snapshot trigger/stack resource bound".into());
        }
        let mut ids = BTreeSet::new();
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
        }
        for entry in &self.state.rules.stack {
            if let Some(id) = self.validate_trigger_entry(entry)?
                && !ids.insert(id)
            {
                return Err("snapshot duplicate trigger identity".into());
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

impl Game {
    fn bind_trigger_entity_targets(
        &mut self,
        placement: TriggerPlacement,
        stack_id: StackId,
    ) -> Result<(), String> {
        let input = self
            .state
            .rules
            .pending
            .take()
            .ok_or("missing trigger entity targets")?;
        let Response::EntityTargets { entities } = input.response else {
            return Err("trigger entity response".into());
        };
        self.validate_entity_targets(input.player, &entities)?;
        let announced = (0..4)
            .map(|v| {
                entities
                    .iter()
                    .map(|t| match t {
                        TargetEntity::Player { .. } => Ok(None),
                        TargetEntity::Object { object } => {
                            self.handle_for(PlayerId(v), *object).map(Some)
                        }
                    })
                    .collect::<Result<Vec<_>, String>>()
            })
            .collect::<Result<Vec<_>, _>>()?;
        let top = self
            .state
            .rules
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
        self.state.rules.frame = Frame::TriggerOrder { placement };
        Ok(())
    }
    pub(super) fn bind_trigger_object_targets(
        &mut self,
        placement: TriggerPlacement,
        stack_id: StackId,
    ) -> Result<(), String> {
        let input = self
            .state
            .rules
            .pending
            .take()
            .ok_or("missing creature-trigger targets")?;
        self.validate_response(input.player, &input.response)?;
        let Response::Targets { objects } = input.response else {
            return Err("creature-trigger response invariant".into());
        };
        let announced = (0..4)
            .map(|v| {
                objects
                    .iter()
                    .map(|o| self.handle_for(PlayerId(v), *o))
                    .collect::<Result<Vec<_>, _>>()
            })
            .collect::<Result<Vec<_>, _>>()?;
        let top = self
            .state
            .rules
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
        self.state.rules.frame = Frame::TriggerOrder { placement };
        Ok(())
    }
}
