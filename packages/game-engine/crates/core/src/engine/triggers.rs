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
    pub(super) match_condition: Option<InterveningCheck>,
    pub(super) id: TriggerId,
    pub(super) source: ObjectId,
    pub(super) controller: PlayerId,
    pub(super) ability: u32,
    pub(super) source_facts: ObjectBefore,
    pub(super) source_handles: Vec<ObjectHandle>,
    event: OccurrenceId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    movement: Option<MoveOutcome>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    token_entered: Option<ObjectId>,
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
        if self.staples_enabled() {
            staples::STACK_BOUND
        } else if self.tokens_enabled() {
            tokens::STACK_BOUND
        } else if self.prepared.config.match_profile() {
            256
        } else if self.prepared.config.triggers_profile() {
            128
        } else {
            4
        }
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
        if !self.prepared.config.triggers_profile() {
            return Ok(());
        }
        for occurrence in occurrences {
            let subjects = if let EventOutcome::TokensCreated { objects, .. } = &occurrence.actual {
                objects.iter().map(|o| (None, Some(*o))).collect::<Vec<_>>()
            } else {
                movement(&occurrence.actual)
                    .map(|m| vec![(Some(m.clone()), None)])
                    .unwrap_or_default()
            };
            for (movement, token_entered) in subjects {
                let dies = movement.as_ref().is_some_and(|m| {
                    m.from == Zone::Battlefield && matches!(m.destination, Zone::Graveyard { .. })
                });
                let enters = token_entered.is_some()
                    || movement.as_ref().is_some_and(|m| {
                        m.destination == Zone::Battlefield && m.from != Zone::Battlefield
                    });
                if !dies && !enters {
                    continue;
                }
                let subject = token_entered
                    .or_else(|| {
                        movement
                            .as_ref()
                            .map(|m| if enters { m.successor } else { m.previous })
                    })
                    .ok_or("missing trigger subject")?;
                if self.definition(subject.card)?.characteristics.kind != CardKind::Creature {
                    continue;
                }
                let observers = if dies { before } else { &*self };
                let mut matched = vec![];
                for object in observers
                    .state
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
                        let matches = match definition.condition {
                            TriggerCondition::SelfDies => dies && object.id == subject,
                            TriggerCondition::AnyCreatureDies => dies,
                            TriggerCondition::SelfEntersBattlefield => {
                                enters && object.id == subject
                            }
                            TriggerCondition::OtherCreatureEntersBattlefield => {
                                enters && object.id != subject
                            }
                        };
                        if matches
                            && definition.functioning_zone == FunctioningZone::Battlefield
                            && observers.is_live(object.controller)
                        {
                            matched.push((
                                observers.object_before(object.id)?,
                                u32::try_from(ability)
                                    .map_err(|_| "trigger ability index limit")?,
                                definition
                                    .intervening_if
                                    .as_ref()
                                    .map(|condition| {
                                        observers.check_intervening(condition, object.controller)
                                    })
                                    .transpose()?,
                            ));
                        }
                    }
                }
                for (source_facts, ability, match_condition) in matched {
                    if let Some(check) = &match_condition {
                        self.record_event(Event::TriggerConditionChecked {
                            source: source_facts.object,
                            controller: source_facts.controller,
                            ability,
                            stage: InterveningStage::Match {
                                occurrence: occurrence.id.clone(),
                            },
                            check: check.clone(),
                        })?;
                        if !check.passed {
                            continue;
                        }
                    }
                    // Historical scenario limits remain unchanged. The tokens
                    // profile uses the complete source-pool bound in tokens.rs.
                    if self.state.pending_triggers.len() + self.state.stack.len()
                        >= self.stack_limit()
                    {
                        if self.tokens_enabled() {
                            let requested =
                                self.state.pending_triggers.len() + self.state.stack.len() + 1;
                            return Err(self.capacity_error(RuleFault::StackCapacityExceeded {
                                capacity: self.stack_limit(),
                                requested,
                            }));
                        }
                        return Err("trigger/stack resource limit".into());
                    }
                    let id = TriggerId(self.state.next_trigger_id);
                    self.state.next_trigger_id = self
                        .state
                        .next_trigger_id
                        .checked_add(1)
                        .ok_or("trigger ID resource limit")?;
                    let source = source_facts.object;
                    let controller = source_facts.controller;
                    let consumed = source_facts.clone();
                    let source_handles = self.capture_trigger_source_handles(source)?;
                    self.state.pending_triggers.push(PendingTrigger {
                        match_condition,
                        id,
                        source,
                        controller,
                        ability,
                        source_facts,
                        source_handles,
                        event: occurrence.id.clone(),
                        movement: movement.clone(),
                        token_entered,
                    });
                    self.record_event(Event::TriggerCollected {
                        trigger_id: id,
                        source,
                        player: controller,
                        ability,
                        occurrence: occurrence.id.clone(),
                        subject,
                        source_facts: Box::new(consumed),
                    })?;
                }
            }
        }
        Ok(())
    }
    fn apnap_players(&self) -> Vec<PlayerId> {
        let anchor = self.state.turn_owner.0;
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
        if !self.prepared.config.triggers_profile() || self.state.pending_triggers.is_empty() {
            return Ok(false);
        }
        let live = self.live_players();
        self.state
            .pending_triggers
            .retain(|t| live.contains(&t.controller));
        if self.state.pending_triggers.is_empty() {
            return Ok(false);
        }
        self.state.frame = Frame::TriggerOrder {
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
            .pending_triggers
            .iter()
            .filter(|t| t.controller == *player)
            .map(|t| t.id)
            .collect()
    }
    pub(super) fn trigger_order_ids(&self) -> Vec<TriggerId> {
        if let Frame::TriggerOrder { placement } = &self.state.frame {
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
        let Frame::TriggerOrder { placement } = &self.state.frame else {
            return Ok(None);
        };
        let candidates = self.trigger_order_ids();
        if candidates.len() > 1 {
            Ok(Some(Request::OrderTriggers {
                decision_id: self.state.revision,
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
            (&self.state.frame, response)
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
        self.state.frame = Frame::Stabilize {
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
        match self.state.frame.clone() {
            Frame::TriggerOrder { mut placement } => {
                if !placement.order.is_empty() {
                    let id = placement.order.remove(0);
                    let index = self
                        .state
                        .pending_triggers
                        .iter()
                        .position(|t| t.id == id)
                        .ok_or("missing ordered trigger")?;
                    let trigger = self.state.pending_triggers.remove(index);
                    self.validate_match_condition(
                        trigger.source,
                        trigger.ability,
                        trigger.controller,
                        trigger.match_condition.as_ref(),
                    )?;
                    if !self.is_live(trigger.controller) {
                        self.state.frame = Frame::TriggerOrder { placement };
                        return Ok(());
                    }
                    let stack_id = StackId(self.state.next_stack_id);
                    self.state.next_stack_id = self
                        .state
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
                    self.state.stack.push(StackInstance {
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
                            subject: trigger
                                .token_entered
                                .or_else(|| {
                                    trigger.movement.as_ref().map(|m| {
                                        if m.destination == Zone::Battlefield {
                                            m.successor
                                        } else {
                                            m.previous
                                        }
                                    })
                                })
                                .ok_or("missing placed trigger subject")?,
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
                    self.state.passes.clear();
                    if targeted {
                        self.state.frame = Frame::TriggerTargets {
                            placement,
                            stack_id,
                        };
                    } else {
                        self.record_event(Event::TriggerPlaced {
                            trigger_id: id,
                            stack_id,
                            player: trigger.controller,
                        })?;
                        self.state.frame = Frame::TriggerOrder { placement };
                    }
                } else if self.trigger_request()?.is_some() {
                    let input = self
                        .state
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
                    self.state.frame = Frame::TriggerOrder { placement };
                } else if placement.remaining.is_empty() {
                    self.finish_trigger_placement(placement)?;
                } else {
                    let candidates = self.group_ids(&placement);
                    if candidates.is_empty() {
                        placement.remaining.remove(0);
                    } else {
                        placement.order = candidates;
                    }
                    self.state.frame = Frame::TriggerOrder { placement };
                }
            }
            Frame::TriggerTargets {
                placement,
                stack_id,
            } => {
                if !self.has_target_domain(self.program()?, self.top()?.controller) {
                    self.state.pending = None;
                    let top = self
                        .state
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
                    self.state.frame = Frame::TriggerOrder { placement };
                    return Ok(());
                }
                if self.program()?.targets.first().is_some_and(|t| {
                    matches!(
                        t.domain,
                        TargetDomain::BattlefieldCreature | TargetDomain::OwnGraveyardCreatureCard
                    )
                }) {
                    return self.bind_trigger_object_targets(placement, stack_id);
                }
                let input = self
                    .state
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
                self.state.frame = Frame::TriggerOrder { placement };
            }
            _ => return Err("trigger placement frame invariant".into()),
        }
        Ok(())
    }
    pub(super) fn revalidate_trigger_departure(&mut self) -> Result<(), String> {
        let live = self.live_players();
        self.state
            .pending_triggers
            .retain(|t| live.contains(&t.controller));
        let revalidate = |mut placement: TriggerPlacement, game: &Game| {
            placement.remaining.retain(|p| game.is_live(*p));
            placement
                .order
                .retain(|id| game.state.pending_triggers.iter().any(|t| t.id == *id));
            placement
        };
        match self.state.frame.clone() {
            Frame::TriggerOrder { placement } => {
                self.state.frame = Frame::TriggerOrder {
                    placement: revalidate(placement, self),
                };
                if self
                    .state
                    .pending
                    .as_ref()
                    .is_some_and(|p| self.validate_response(p.player, &p.response).is_err())
                {
                    self.state.pending = None;
                }
            }
            Frame::TriggerTargets {
                placement,
                stack_id,
            } => {
                let placement = revalidate(placement, self);
                if !self.state.stack.iter().any(|s| s.id == stack_id) {
                    self.state.pending = None;
                    self.state.frame = Frame::TriggerOrder { placement };
                } else {
                    self.state.frame = Frame::TriggerTargets {
                        placement,
                        stack_id,
                    };
                    if self
                        .state
                        .pending
                        .as_ref()
                        .is_some_and(|p| self.validate_response(p.player, &p.response).is_err())
                    {
                        self.state.pending = None;
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
        if !self.prepared.config.triggers_profile()
            || controller.0 >= 4
            || fact.object != source
            || fact.owner
                != self
                    .state
                    .cards
                    .get(&source.card)
                    .ok_or("snapshot unknown trigger source")?
                    .owner
            || fact.controller.0 >= 4
            || fact.default_controller.0 >= 4
            || fact.zone != Zone::Battlefield
            || (!self.prepared.config.effects_profile()
                && fact.characteristics != self.definition(source.card)?.characteristics)
            || handles.len() != 4
            || definition.functioning_zone != FunctioningZone::Battlefield
        {
            return Err("snapshot trigger source invariant".into());
        }
        if self.prepared.config.effects_profile() {
            self.validate_characteristic_identity(source, &fact.characteristics)?;
        }
        Ok(())
    }
    pub(super) fn validate_trigger_state(&self) -> Result<(), String> {
        if !self.prepared.config.triggers_profile() {
            if self.state.next_trigger_id != 0
                || !self.state.pending_triggers.is_empty()
                || matches!(
                    self.state.frame,
                    Frame::TriggerOrder { .. } | Frame::TriggerTargets { .. }
                )
                || self
                    .state
                    .stack
                    .iter()
                    .any(|s| matches!(s.kind, StackKind::Triggered { .. }))
            {
                return Err("snapshot unexpected trigger state".into());
            }
            return Ok(());
        }
        if self.state.pending_triggers.len() + self.state.stack.len() > self.stack_limit() {
            return Err("snapshot trigger/stack resource bound".into());
        }
        let mut ids = BTreeSet::new();
        for trigger in &self.state.pending_triggers {
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
                || trigger.id.0 >= self.state.next_trigger_id
                || !ids.insert(trigger.id)
                || trigger.event.batch >= self.state.next_batch_id
                || (trigger.movement.is_some() == trigger.token_entered.is_some())
                || trigger.movement.as_ref().is_some_and(|m| {
                    m.successor.card != m.previous.card
                        || m.previous.incarnation.checked_add(1) != Some(m.successor.incarnation)
                })
                || trigger
                    .token_entered
                    .is_some_and(|o| !self.state.tokens.contains_key(&o.card) || o.incarnation != 0)
            {
                return Err("snapshot pending trigger identity invariant".into());
            }
            let condition =
                self.definition(trigger.source.card)?.triggered[trigger.ability as usize].condition;
            let valid = if let Some(token) = trigger.token_entered {
                condition == TriggerCondition::OtherCreatureEntersBattlefield
                    && token != trigger.source
            } else if let Some(m) = &trigger.movement {
                (match condition {
                    TriggerCondition::SelfDies => {
                        m.from == Zone::Battlefield
                            && matches!(m.destination, Zone::Graveyard { .. })
                            && m.previous == trigger.source
                    }
                    TriggerCondition::AnyCreatureDies => {
                        m.from == Zone::Battlefield
                            && matches!(m.destination, Zone::Graveyard { .. })
                    }
                    TriggerCondition::SelfEntersBattlefield => {
                        m.destination == Zone::Battlefield
                            && m.from != Zone::Battlefield
                            && m.successor == trigger.source
                    }
                    TriggerCondition::OtherCreatureEntersBattlefield => {
                        m.destination == Zone::Battlefield
                            && m.from != Zone::Battlefield
                            && m.successor != trigger.source
                    }
                }) && self.definition(m.previous.card)?.characteristics.kind == CardKind::Creature
            } else {
                false
            };
            if !valid {
                return Err("snapshot trigger event invariant".into());
            }
        }
        for entry in &self.state.stack {
            let StackKind::Triggered {
                ability,
                source_handles,
                trigger_id,
                event,
                subject,
                match_condition,
            } = &entry.kind
            else {
                continue;
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
            if trigger_id.0 >= self.state.next_trigger_id
                || !ids.insert(*trigger_id)
                || event.batch >= self.state.next_batch_id
                || !self.state.cards.contains_key(&subject.card)
                || entry.cast_from != Zone::Battlefield
                || (!entry.targets.is_empty()
                    && self.program_for(entry)?.targets.first().is_none_or(|t| {
                        !matches!(
                            t.domain,
                            TargetDomain::BattlefieldCreature
                                | TargetDomain::OwnGraveyardCreatureCard
                        )
                    }))
                || !entry.entity_targets.is_empty()
                || !entry.stack_targets.is_empty()
                || entry.total_cost
                    != (ManaCost {
                        generic: Quantity::zero(),
                        colored: vec![],
                    })
            {
                return Err("snapshot triggered invocation invariant".into());
            }
        }
        if let Frame::TriggerOrder { placement } | Frame::TriggerTargets { placement, .. } =
            &self.state.frame
        {
            if placement.remaining.iter().any(|p| !self.is_live(*p))
                || placement.remaining.iter().collect::<BTreeSet<_>>().len()
                    != placement.remaining.len()
                || placement.order.iter().collect::<BTreeSet<_>>().len() != placement.order.len()
                || placement.order.iter().any(|id| {
                    !self
                        .state
                        .pending_triggers
                        .iter()
                        .any(|t| t.id == *id && Some(&t.controller) == placement.remaining.first())
                })
            {
                return Err("snapshot APNAP placement invariant".into());
            }
            if let Frame::TriggerTargets { stack_id, .. } = &self.state.frame {
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
    fn bind_trigger_object_targets(
        &mut self,
        placement: TriggerPlacement,
        stack_id: StackId,
    ) -> Result<(), String> {
        let input = self
            .state
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
        self.state.frame = Frame::TriggerOrder { placement };
        Ok(())
    }
}
