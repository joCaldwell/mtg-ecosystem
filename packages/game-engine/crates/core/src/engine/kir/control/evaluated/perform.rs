//! Hand evaluated operands to the specialist authorities: the damage
//! transaction (CR 120), counter placement (CR 122.6) and the graveyard group
//! that performs a sacrifice (CR 701.21a). Each transaction resumes the saved
//! task with its actual result.
use super::*;
use mtg_contracts::kir::VerbSignature;

impl RulesTxn {
    pub(super) fn perform_step(
        &mut self,
        task: Task,
        signature: &'static VerbSignature,
        operands: &Evaluated<'_>,
        amount: Quantity,
        block: u32,
        empty: InstructionOutcome,
    ) -> Result<(), String> {
        let performed = StepResult {
            amount: Some(Quantity::zero()),
            ..StepResult::default()
        };
        match (signature.distribution, *operands) {
            (Distribution::EachSimultaneous, Evaluated::Damage { source, to, .. }) => {
                let Some((facts, controller)) = self.damage_source_facts(&task, source)? else {
                    return self.complete_evaluated(&task, None, [empty]);
                };
                let recipients = self.evaluated_recipients(&task, to)?;
                if amount == Quantity::zero() || recipients.is_empty() {
                    return self.complete_evaluated(&task, Some(performed), [empty]);
                }
                self.begin_kernel_damage(task, facts, controller, recipients, amount, block)
            }
            (Distribution::EachSimultaneous, Evaluated::Counters { on, counter, .. }) => {
                let recipients = self.counter_recipients(&task, on, counter)?;
                if amount == Quantity::zero() || recipients.is_empty() {
                    return self.complete_evaluated(&task, Some(performed), [empty]);
                }
                self.begin_kernel_counters(task, counter.clone(), recipients, amount, block)
            }
            (Distribution::CollectThenSimultaneous, Evaluated::Sacrifice { player, .. }) => {
                let recipient = &signature.slots[0];
                let Some(players) =
                    Self::present_operand(recipient, self.evaluated_players(&task, player)?)?
                else {
                    return self.complete_evaluated(&task, None, [empty]);
                };
                if amount == Quantity::zero() || players.is_empty() {
                    return self.complete_evaluated(&task, Some(performed), [empty]);
                }
                if let Some(object) = operands.designated() {
                    // CR 701.21a: a player sacrifices only a permanent they
                    // control; a new incarnation is not "it" (CR 400.7).
                    let Resolved::Present(object) = self.evaluated_object(&task, object)? else {
                        return self.complete_evaluated(&task, Some(performed), [empty]);
                    };
                    let chosen: Vec<_> = players
                        .into_iter()
                        .filter(|p| self.sacrificeable(*p, object))
                        .take(1)
                        .map(|p| (p, object))
                        .collect();
                    return self.commit_kernel_sacrifice(task, chosen, block);
                }
                let mut task = task;
                task.stage = Stage::Step(StepProgress::Collect {
                    remaining: players,
                    chosen: vec![],
                    count: amount,
                });
                self.install_kernel(task);
                Ok(())
            }
            _ => Err("unsupported evaluated distribution".into()),
        }
    }

    /// CR 120.1, 608.2h: the object that deals the damage, with its current
    /// facts or its last-known information once it has left, and its
    /// controller. An illegal or departed target deals no damage (CR 608.2b).
    pub(in crate::engine::kir::control) fn damage_source_facts(
        &self,
        task: &Task,
        source: &Ref,
    ) -> Result<Option<(ObjectBefore, PlayerId)>, String> {
        let entry = self.entry(self.kernel_root()?.stack_id)?;
        Ok(Some(match source {
            Ref::Source => (self.source_facts(entry)?, entry.controller),
            Ref::EventObject { .. } => {
                let binding = entry.kind.event_binding()?;
                let Some(facts) = binding.object.as_deref() else {
                    return Ok(None);
                };
                let facts = self
                    .current_source_facts(facts.object)
                    .unwrap_or_else(|_| binding.last_known.as_deref().unwrap_or(facts).clone());
                let controller = facts.controller;
                (facts, controller)
            }
            Ref::Target { .. } | Ref::BoundVar { .. } => {
                let Resolved::Present(object) = self.evaluated_object(task, source)? else {
                    return Ok(None);
                };
                if !self
                    .current(object)
                    .is_ok_and(|o| o.id == object && o.present_on_battlefield())
                {
                    return Ok(None);
                }
                let facts = self.object_before(object)?;
                let controller = facts.controller;
                (facts, controller)
            }
            _ => return Err("unsupported damage source".into()),
        }))
    }

    /// CR 120.1, 120.3, 608.2b: the players and permanents the step deals
    /// damage to, each once, in written order. A player who has left or an
    /// object that is no longer a creature or planeswalker is skipped.
    pub(in crate::engine::kir::control) fn evaluated_recipients(
        &self,
        task: &Task,
        to: &DamageTo,
    ) -> Result<Vec<TargetEntity>, String> {
        let mut out = Vec::new();
        self.collect_recipients(task, to, &mut out)?;
        Ok(out)
    }

    fn collect_recipients(
        &self,
        task: &Task,
        to: &DamageTo,
        out: &mut Vec<TargetEntity>,
    ) -> Result<(), String> {
        let frame = self.kernel_root()?;
        let entry = self.entry(frame.stack_id)?;
        let push = |recipient: TargetEntity, out: &mut Vec<TargetEntity>| {
            let legal = match recipient {
                TargetEntity::Player { player } => self.is_live(player),
                TargetEntity::Object { object } => self.is_permanent_damage_recipient(object),
            };
            if legal && !out.contains(&recipient) {
                out.push(recipient);
            }
        };
        match to {
            DamageTo::Ref {
                who: Ref::Target { slot },
            } => {
                if frame.legal_targets.get(*slot as usize) == Some(&true) {
                    let target = self.directed_damage_target(entry, *slot)?;
                    if self.entity_legal(target) {
                        push(target, out);
                    }
                }
            }
            DamageTo::Ref {
                who: who @ (Ref::Controller { .. } | Ref::EventPlayer { .. }),
            } => {
                if let Resolved::Present(players) =
                    self.evaluated_players(task, &PlayerExpr::Is { who: *who })?
                {
                    for player in players {
                        push(TargetEntity::Player { player }, out);
                    }
                }
            }
            DamageTo::Ref {
                who: Ref::BoundVar { id },
            } if matches!(task.bindings.get(id), Some(Binding::Player(_))) => {
                let Some(Binding::Player(player)) = task.bindings.get(id) else {
                    unreachable!("bound player");
                };
                push(TargetEntity::Player { player: *player }, out);
            }
            DamageTo::Ref { who } => {
                if let Resolved::Present(object) = self.evaluated_object(task, who)?
                    && self.current(object).is_ok_and(|o| o.id == object)
                {
                    push(TargetEntity::Object { object }, out);
                }
            }
            DamageTo::Players { players } => {
                if let Resolved::Present(players) = self.evaluated_players(task, players)? {
                    for player in players {
                        push(TargetEntity::Player { player }, out);
                    }
                }
            }
            DamageTo::Objects { set } => {
                if let Resolved::Present(objects) = self.evaluated_objects(task, set)? {
                    for object in objects {
                        if self.current(object).is_ok_and(|o| o.id == object) {
                            push(TargetEntity::Object { object }, out);
                        }
                    }
                }
            }
            DamageTo::All { parts } => {
                for part in parts {
                    self.collect_recipients(task, part, out)?;
                }
            }
        }
        Ok(())
    }

    /// The handles every viewer saw for the damage source when the invocation
    /// began, or handles for another public object that deals the damage.
    fn damage_source_handles(
        &mut self,
        source: &ObjectBefore,
    ) -> Result<Vec<publish::StoredObjectHandle>, String> {
        let entry = self.entry(self.kernel_root()?.stack_id)?;
        if source.object == entry.source {
            return Ok(match &entry.kind {
                StackKind::Activated { source_handles, .. }
                | StackKind::Triggered { source_handles, .. } => source_handles.clone(),
                StackKind::Spell => entry.effect_source_handles.clone(),
            });
        }
        if let StackKind::Triggered { witness, .. } = &entry.kind
            && let Some(observed) = witness.observed_object()
            && observed.last_known.object == source.object
        {
            return Ok(observed.handles.clone());
        }
        self.capture_trigger_source_handles(source.object)
    }

    fn begin_kernel_damage(
        &mut self,
        mut task: Task,
        source: ObjectBefore,
        controller: PlayerId,
        recipients: Vec<TargetEntity>,
        amount: Quantity,
        block: u32,
    ) -> Result<(), String> {
        let handles = self.damage_source_handles(&source)?;
        let entries = recipients
            .iter()
            .enumerate()
            .map(|(n, recipient)| effects::PreparedDamage {
                source_appearance: None,
                source_handles: handles.clone(),
                id: DamageId(n as u32),
                source: source.clone(),
                controller,
                recipient: match *recipient {
                    TargetEntity::Player { player } => DamageRecipient::Player { player },
                    TargetEntity::Object { object } => DamageRecipient::Permanent { object },
                },
                original_amount: amount.clone(),
                amount: amount.clone(),
                applied: vec![],
            })
            .collect();
        let cause = self.instruction_cause(self.kernel_root()?, block)?;
        task.stage = Stage::Step(StepProgress::Damage { recipients, amount });
        let resume = self.save_frame(Frame::Kernel {
            task: Box::new(task),
        });
        self.begin_damage(
            DamageKind::Noncombat,
            entries,
            cause,
            DamageContinuation::Kernel { resume },
        )
    }

    /// CR 120.3, 120.4: record the damage actually dealt, after prevention.
    pub(in crate::engine) fn finish_kernel_damage(
        &mut self,
        resume: FrameLink,
        batch: &CommittedBatch,
    ) -> Result<(), String> {
        let Frame::Kernel { task } = self.saved_frame(resume)?.clone() else {
            return Err("missing kernel damage task".into());
        };
        let [receipt] = batch.occurrences.as_slice() else {
            return Err("unexpected kernel damage receipt".into());
        };
        let EventOutcome::DamageCommitted { actual, .. } = &receipt.actual else {
            return Err("unexpected kernel damage outcome".into());
        };
        let mut result = StepResult {
            events: vec![receipt.id.clone()],
            amount: Some(Quantity::zero()),
            ..StepResult::default()
        };
        for damage in actual.iter().filter(|d| d.amount > Quantity::zero()) {
            result
                .amount
                .as_mut()
                .expect("damage projection")
                .add(&damage.amount);
            match damage.recipient {
                DamageRecipient::Player { player } => {
                    if !result.players.contains(&player) {
                        result.players.push(player);
                    }
                }
                DamageRecipient::Permanent { object } => {
                    if !result.objects.iter().any(|o| o.prior == object) {
                        result.objects.push(ResultObject {
                            prior: object,
                            successor: None,
                            controller_before: self.current(object)?.controller,
                        });
                    }
                }
            }
        }
        let outcome = InstructionOutcome::Damage {
            actual: Some(receipt.actual.clone()),
        };
        self.complete_evaluated(&task, Some(result), [outcome])
    }

    /// A damage transaction that resumes a task deals the damage its saved
    /// step read: the same source, amount and a subset of its recipients.
    pub(in crate::engine) fn validate_kernel_damage(
        &self,
        tx: &DamageTransaction,
        resume: FrameLink,
    ) -> Result<(), String> {
        let Frame::Kernel { task } = self.saved_frame(resume)? else {
            return Err("missing kernel damage task".into());
        };
        let Stage::Step(StepProgress::Damage { recipients, amount }) = &task.stage else {
            return Err("kernel damage task is not dealing damage".into());
        };
        let (node, block) = self.node_at(task)?;
        let Some((
            _,
            Evaluated::Damage {
                source,
                to,
                amount: expression,
            },
        )) = node.verb().and_then(Verb::evaluated)
        else {
            return Err("kernel damage task does not name damage".into());
        };
        let frame = self.kernel_root()?;
        if tx.kind != DamageKind::Noncombat
            || tx.cause != self.instruction_cause(frame, block)?
            || *amount <= Quantity::zero()
            || recipients.is_empty()
            || recipients.len() > self.damage_limit()
            || recipients.iter().collect::<BTreeSet<_>>().len() != recipients.len()
            || tx.entries.len() != recipients.len()
        {
            return Err("invalid kernel damage transaction".into());
        }
        let invocation = self.entry(frame.stack_id)?;
        let (source_id, controller) = if matches!(source, Ref::Source) {
            (Some(invocation.source), Some(invocation.controller))
        } else {
            (
                match self.evaluated_object(task, source)? {
                    Resolved::Present(object) => Some(object),
                    Resolved::Absent(_) => None,
                },
                None,
            )
        };
        for (n, (entry, recipient)) in tx.entries.iter().zip(recipients).enumerate() {
            if entry.id != DamageId(n as u32)
                || Some(entry.source.object) != source_id
                || entry.controller != controller.unwrap_or(entry.source.controller)
                || entry.original_amount != *amount
                || entry.recipient.entity() != *recipient
            {
                return Err("kernel damage entry differs from its step".into());
            }
        }
        // CR 800.4a: removed seats and incarnations may still occur in the
        // proposal. The live portion must still agree with the step's scope.
        let surviving = recipients
            .iter()
            .copied()
            .filter(|recipient| match recipient {
                TargetEntity::Player { player } => self.is_live(*player),
                TargetEntity::Object { object } => self.is_permanent_damage_recipient(*object),
            });
        if !surviving.eq(self.evaluated_recipients(task, to)?) {
            return Err("kernel damage recipient differs from its step".into());
        }
        let departure = self
            .state
            .rules
            .original_seating
            .iter()
            .any(|p| !self.is_live(*p));
        if departure && expression.is_resolution_count() {
            if crate::engine::kir::resolution_count_bound(expression)
                .is_some_and(|bound| *amount > bound)
            {
                return Err("kernel damage amount exceeds its step's bound".into());
            }
        } else if matches!(expression, Amount::Fixed { .. } | Amount::Var { .. })
            && self.evaluated_amount(task, expression)? != Resolved::Present(amount.clone())
        {
            return Err("kernel damage amount differs from its step".into());
        }
        Ok(())
    }

    /// CR 122.1, 122.6: the live permanents that can receive this kind now.
    pub(in crate::engine::kir::control) fn counter_recipients(
        &self,
        task: &Task,
        on: &SetExpr,
        counter: &kir::CounterKind,
    ) -> Result<Vec<ObjectId>, String> {
        let Resolved::Present(objects) = self.evaluated_objects(task, on)? else {
            return Ok(vec![]);
        };
        let mut out = Vec::new();
        for object in objects {
            if self.current(object).is_ok_and(|o| o.id == object)
                && self.counter_recipient(object, counter)
                && !out.contains(&object)
            {
                out.push(object);
            }
        }
        Ok(out)
    }

    fn begin_kernel_counters(
        &mut self,
        mut task: Task,
        counter: kir::CounterKind,
        recipients: Vec<ObjectId>,
        amount: Quantity,
        block: u32,
    ) -> Result<(), String> {
        let cause = self.instruction_cause(self.kernel_root()?, block)?;
        let allocations = recipients.iter().map(|o| (*o, amount.clone())).collect();
        task.stage = Stage::Step(StepProgress::Counters { recipients, amount });
        let resume = self.save_frame(Frame::Kernel {
            task: Box::new(task),
        });
        let frame = self.kernel_root()?.clone();
        self.begin_kernel_counter_placements(frame, counter, allocations, cause, resume)
    }

    /// CR 122.6: record the counters actually put on each permanent.
    pub(in crate::engine) fn finish_kernel_counters(
        &mut self,
        resume: FrameLink,
        batch: Option<&CommittedBatch>,
        outcome: InstructionOutcome,
    ) -> Result<(), String> {
        let Frame::Kernel { task } = self.saved_frame(resume)?.clone() else {
            return Err("missing kernel counter task".into());
        };
        let mut result = StepResult {
            amount: Some(Quantity::zero()),
            ..StepResult::default()
        };
        for receipt in batch.into_iter().flat_map(|batch| &batch.occurrences) {
            result.events.push(receipt.id.clone());
            if let EventOutcome::CountersPlaced { object, placed } = &receipt.actual {
                for placed in placed {
                    result
                        .amount
                        .as_mut()
                        .expect("counter projection")
                        .add(&placed.amount);
                }
                if !result.objects.iter().any(|o| o.prior == *object) {
                    result.objects.push(ResultObject {
                        prior: *object,
                        successor: None,
                        controller_before: self.current(*object)?.controller,
                    });
                }
            }
        }
        self.complete_evaluated(&task, Some(result), [outcome])
    }

    /// The counter placement a task saved is the one its step read.
    pub(in crate::engine) fn validate_kernel_counters(
        &self,
        members: &[crate::engine::counter_placement::CounterPlacementMember],
        cause: &BatchCause,
        resume: FrameLink,
    ) -> Result<(), String> {
        let Frame::Kernel { task } = self.saved_frame(resume)? else {
            return Err("missing kernel counter task".into());
        };
        let Stage::Step(StepProgress::Counters { recipients, amount }) = &task.stage else {
            return Err("kernel counter task is not placing counters".into());
        };
        let (node, block) = self.node_at(task)?;
        let Some((_, Evaluated::Counters { counter, .. })) = node.verb().and_then(Verb::evaluated)
        else {
            return Err("kernel counter task does not name counters".into());
        };
        if *cause != self.instruction_cause(self.kernel_root()?, block)?
            || *amount <= Quantity::zero()
            || members.len() != recipients.len()
            || members
                .iter()
                .zip(recipients)
                .any(|(m, r)| m.object != *r || m.counter != *counter || m.amount != *amount)
        {
            return Err("kernel counter placement differs from its step".into());
        }
        Ok(())
    }

    /// CR 701.21a: whether `player` can sacrifice `object` now.
    fn sacrificeable(&self, player: PlayerId, object: ObjectId) -> bool {
        self.is_live(player)
            && self.current(object).is_ok_and(|o| {
                o.id == object && o.present_on_battlefield() && o.controller == player
            })
    }

    /// CR 701.21a, 608.2h: the permanents `player` may choose to sacrifice:
    /// those they control that match the filter now, or only the ones with
    /// the greatest power among them, every tie included.
    pub(in crate::engine::kir::control) fn kernel_sacrifice_candidates(
        &self,
        task: &Task,
        operands: &Evaluated<'_>,
        player: PlayerId,
    ) -> Result<Vec<ObjectId>, String> {
        let Evaluated::Sacrifice {
            filter,
            greatest_power,
            ..
        } = *operands
        else {
            return Err("sacrifice candidates outside a sacrifice".into());
        };
        if !self.is_live(player) {
            return Ok(vec![]);
        }
        if let Some(object) = operands.designated() {
            return Ok(match self.evaluated_object(task, object)? {
                Resolved::Present(object) if self.sacrificeable(player, object) => vec![object],
                _ => vec![],
            });
        }
        let options: Vec<ObjectId> = self
            .evaluated_set(task, &ZoneExpr::Battlefield, filter)?
            .into_iter()
            .filter(|object| self.sacrificeable(player, *object))
            .collect();
        if !greatest_power {
            return Ok(options);
        }
        let mut powers = Vec::new();
        for object in options {
            let power = self.characteristics(object)?.power.unwrap_or_default();
            powers.push((object, power));
        }
        let Some(greatest) = powers.iter().map(|(_, power)| power.clone()).max() else {
            return Ok(vec![]);
        };
        Ok(powers
            .into_iter()
            .filter(|(_, power)| *power == greatest)
            .map(|(object, _)| object)
            .collect())
    }

    pub(in crate::engine::kir::control) fn collect_request(
        &self,
        task: &Task,
    ) -> Result<Option<Decision<Rules>>, String> {
        let Stage::Step(StepProgress::Collect {
            remaining,
            chosen,
            count,
        }) = &task.stage
        else {
            return Ok(None);
        };
        let Some(&player) = remaining.first().filter(|p| self.is_live(**p)) else {
            return Ok(None);
        };
        let (node, _) = self.node_at(task)?;
        let Some((_, operands @ Evaluated::Sacrifice { player: scope, .. })) =
            node.verb().and_then(Verb::evaluated)
        else {
            return Err("missing sacrifice".into());
        };
        let options = self.kernel_sacrifice_candidates(task, &operands, player)?;
        let count = (count.to_u32().unwrap_or(u32::MAX) as usize).min(options.len());
        let reason = if matches!(scope, PlayerExpr::EachPlayer | PlayerExpr::EachOpponent) {
            Reason::ForcedSacrifice {
                previous: chosen.clone(),
            }
        } else {
            Reason::Sacrifice
        };
        Ok((count != 0).then(|| self.pick_objects(player, reason, options, count)))
    }

    /// CR 101.4: take the next player's choice, then sacrifice every chosen
    /// permanent still controlled by its chooser in one event.
    pub(in crate::engine::kir::control) fn step_collect(
        &mut self,
        mut task: Task,
    ) -> Result<(), String> {
        let (node, block) = self.node_at(&task)?;
        let node = node.clone();
        let (_, operands) = node
            .verb()
            .and_then(Verb::evaluated)
            .ok_or("missing sacrifice")?;
        let Stage::Step(StepProgress::Collect {
            mut remaining,
            mut chosen,
            count,
        }) = task.stage.clone()
        else {
            return Err("missing sacrifice collection".into());
        };
        let Some(player) = remaining.first().copied() else {
            let chosen = chosen
                .into_iter()
                .filter(|(player, object)| self.sacrificeable(*player, *object))
                .collect();
            return self.commit_kernel_sacrifice(task, chosen, block);
        };
        let input = self.rules_mut().pending.take();
        let options = self.kernel_sacrifice_candidates(&task, &operands, player)?;
        if !options.is_empty() {
            let objects = input.ok_or("missing forced sacrifice input")?.objects()?;
            let wanted = (count.to_u32().unwrap_or(u32::MAX) as usize).min(options.len());
            if objects.len() != wanted
                || objects.iter().any(|o| !options.contains(o))
                || objects.iter().collect::<BTreeSet<_>>().len() != objects.len()
            {
                return Err("ineligible forced sacrifice".into());
            }
            chosen.extend(objects.into_iter().map(|object| (player, object)));
        }
        remaining.remove(0);
        task.stage = Stage::Step(StepProgress::Collect {
            remaining,
            chosen,
            count,
        });
        self.install_kernel(task);
        Ok(())
    }

    fn commit_kernel_sacrifice(
        &mut self,
        mut task: Task,
        chosen: Vec<(PlayerId, ObjectId)>,
        block: u32,
    ) -> Result<(), String> {
        if chosen.is_empty() {
            let result = StepResult {
                amount: Some(Quantity::zero()),
                ..StepResult::default()
            };
            return self.complete_evaluated(
                &task,
                Some(result),
                [InstructionOutcome::Effect { actual: None }],
            );
        }
        let mut occurrences: Vec<_> = chosen
            .iter()
            .map(|&(player, object)| ProposedEvent::Sacrifice { player, object })
            .collect();
        let group = if occurrences.len() == 1 {
            EventGroup::Occurrence {
                proposed: occurrences.remove(0),
            }
        } else {
            EventGroup::Simultaneous { occurrences }
        };
        let cause = self.instruction_cause(self.kernel_root()?, block)?;
        task.stage = Stage::Step(StepProgress::Sacrifice { chosen });
        let resume = self.save_frame(Frame::Kernel {
            task: Box::new(task),
        });
        self.complete_graveyard_group(cause, group, GroupFinish::Kernel { resume })
    }

    /// CR 701.21a: record the permanents actually sacrificed and who did.
    pub(in crate::engine) fn finish_kernel_sacrifice(
        &mut self,
        resume: FrameLink,
        batch: Option<&CommittedBatch>,
    ) -> Result<(), String> {
        let Frame::Kernel { task } = self.saved_frame(resume)?.clone() else {
            return Err("missing kernel sacrifice task".into());
        };
        let mut result = StepResult {
            amount: Some(Quantity::zero()),
            ..StepResult::default()
        };
        let mut outcomes = Vec::new();
        for receipt in batch.into_iter().flat_map(|batch| &batch.occurrences) {
            if let EventOutcome::Sacrificed { player, movement } = &receipt.actual {
                result.events.push(receipt.id.clone());
                result
                    .amount
                    .as_mut()
                    .expect("sacrifice projection")
                    .add(&Quantity::from(1));
                result.objects.push(ResultObject {
                    prior: movement.previous,
                    successor: Some(movement.successor),
                    controller_before: *player,
                });
                if !result.players.contains(player) {
                    result.players.push(*player);
                }
                outcomes.push(InstructionOutcome::Sacrificed {
                    player: *player,
                    movement: Some(movement.clone()),
                });
            }
        }
        if outcomes.is_empty() {
            outcomes.push(InstructionOutcome::Effect { actual: None });
        }
        self.complete_evaluated(&task, Some(result), outcomes)
    }

    /// The sacrifice group a task saved names the permanents its step chose.
    pub(in crate::engine) fn validate_kernel_sacrifice(
        &self,
        cause: &BatchCause,
        leaves: &[ProposedEvent],
        resume: FrameLink,
    ) -> Result<bool, String> {
        let Frame::Kernel { task } = self.saved_frame(resume)? else {
            return Err("missing kernel sacrifice task".into());
        };
        let Stage::Step(StepProgress::Sacrifice { chosen }) = &task.stage else {
            return Err("kernel sacrifice task is not sacrificing".into());
        };
        let (node, block) = self.node_at(task)?;
        if !matches!(node, Node::Do(Verb::Sacrifice { .. })) {
            return Err("kernel sacrifice task does not name a sacrifice".into());
        }
        Ok(*cause == self.instruction_cause(self.kernel_root()?, block)?
            && leaves.len() == chosen.len()
            && leaves.iter().zip(chosen).all(|(leaf, (p, o))| {
                matches!(leaf, ProposedEvent::Sacrifice { player, object } if player == p && object == o)
            }))
    }

    pub(in crate::engine::kir::control) fn validate_collect(
        &self,
        task: &Task,
        progress: &StepProgress,
    ) -> Result<(), String> {
        let StepProgress::Collect {
            remaining,
            chosen,
            count,
        } = progress
        else {
            return Err("missing sacrifice collection".into());
        };
        let (node, _) = self.node_at(task)?;
        let Some((_, operands @ Evaluated::Sacrifice { player, .. })) =
            node.verb().and_then(Verb::evaluated)
        else {
            return Err("collection outside a sacrifice".into());
        };
        let seating = &self.state.rules.original_seating;
        let order = self.apnap_seats().collect::<Vec<_>>();
        let position = |p: &PlayerId| order.iter().position(|q| q == p);
        let per_player = count.to_u32().unwrap_or(u32::MAX) as usize;
        if *count <= Quantity::zero()
            || operands.designated().is_some()
            || remaining
                .iter()
                .chain(chosen.iter().map(|(p, _)| p))
                .any(|p| !seating.contains(p))
            || remaining
                .windows(2)
                .any(|w| position(&w[0]) >= position(&w[1]))
            || chosen.iter().any(|(p, _)| remaining.contains(p))
            || chosen
                .chunk_by(|a, b| a.0 == b.0)
                .any(|group| group.len() > per_player)
            || chosen.iter().map(|(_, o)| o).collect::<BTreeSet<_>>().len() != chosen.len()
            || chosen
                .iter()
                .any(|(_, o)| !self.state.rules.cards.contains_key(&o.card))
        {
            return Err("invalid sacrifice collection".into());
        }
        if let Resolved::Present(players) = self.evaluated_players(task, player)?
            && remaining
                .iter()
                .any(|p| !players.contains(p) && self.is_live(*p))
        {
            return Err("sacrifice collection names another player".into());
        }
        Ok(())
    }
}
