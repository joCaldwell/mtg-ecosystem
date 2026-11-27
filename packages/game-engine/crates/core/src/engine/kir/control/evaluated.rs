//! Evaluate the checked operands once, then apply the verb's distribution.
use super::*;
use mtg_contracts::kir::{
    self, Absent, AbsentReason, Distribution, EvaluatedVerb, OperandSlot, QuantityDomain, Resolved,
    Stat, StepRole,
};

#[derive(Clone, Debug, PartialEq, Eq)]
struct StepInstance {
    stack: StackId,
    instruction: u32,
    path: Vec<u32>,
    iterations: Vec<u32>,
}

type StepKey = (Vec<u32>, Vec<u32>);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ResultObject {
    prior: ObjectId,
    successor: Option<ObjectId>,
    controller_before: PlayerId,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StepResult {
    events: Vec<OccurrenceId>,
    objects: Vec<ResultObject>,
    players: Vec<PlayerId>,
    amount: Option<Quantity>,
}

// JSON cannot encode the compound addresses as map keys.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(try_from = "Vec<(StepKey, StepResult)>")]
pub(super) struct StepResults(BTreeMap<StepKey, StepResult>);

impl Serialize for StepResults {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(&self.0)
    }
}

impl TryFrom<Vec<(StepKey, StepResult)>> for StepResults {
    type Error = &'static str;
    fn try_from(rows: Vec<(StepKey, StepResult)>) -> Result<Self, Self::Error> {
        let mut map = BTreeMap::new();
        for (key, result) in rows {
            if map.insert(key, result).is_some() {
                return Err("duplicate step result address");
            }
        }
        Ok(Self(map))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "progress", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum StepProgress {
    Sequence {
        players: Vec<PlayerDrawAttempts>,
        at: usize,
        total: Quantity,
        remaining: Quantity,
        result: StepResult,
        last: Option<InstructionOutcome>,
    },
}

impl RulesTxn {
    fn step_instance(&self, task: &Task) -> Result<StepInstance, String> {
        let root = self.kernel_root()?;
        Ok(StepInstance {
            stack: root.stack_id,
            instruction: root.cursor,
            path: task.path.clone(),
            iterations: task.iterations.clone(),
        })
    }

    fn step_collector(&self) -> Result<(&StepResults, u64), String> {
        let Some(Frame::Kernel { task }) = self.state.rules.frames.get(1) else {
            return Err("missing step collector".into());
        };
        match &task.stage {
            Stage::Complete {
                results,
                first_batch,
                ..
            } => Ok((results, *first_batch)),
            _ => Err("invalid step collector".into()),
        }
    }

    fn complete_evaluated(
        &mut self,
        task: &Task,
        result: Option<StepResult>,
        outcomes: impl IntoIterator<Item = InstructionOutcome>,
    ) -> Result<(), String> {
        if let Some(result) = result {
            let step = self.step_instance(task)?;
            let Some(Frame::Kernel { task: root }) = self.rules_mut().frames.get_mut(1) else {
                return Err("missing step collector".into());
            };
            let Stage::Complete { results, .. } = &mut root.stage else {
                return Err("invalid step collector".into());
            };
            if results
                .0
                .insert((step.path, step.iterations), result)
                .is_some()
            {
                return Err("step already performed".into());
            }
        }
        self.kernel_results()?.extend(outcomes);
        self.return_kernel(task.resume)
    }

    fn read_step_result(&self, task: &Task, step: u32) -> Result<Option<&StepResult>, String> {
        let mut parent = task.clone();
        let current = parent.path.pop().ok_or("step result outside sequence")?;
        let (Node::Seq(nodes), _) = self.node_at(&parent)? else {
            return Err("step result outside sequence".into());
        };
        if step >= current {
            return Err("step result does not dominate its reader".into());
        }
        let (_, child) = nodes
            .get(step as usize)
            .and_then(kir::step_producer)
            .ok_or("missing step producer")?;
        parent.path.push(step);
        parent.path.extend(child);
        Ok(self
            .step_collector()?
            .0
            .0
            .get(&(parent.path, parent.iterations)))
    }

    fn present_operand<T>(slot: &OperandSlot, value: Resolved<T>) -> Result<Option<T>, String> {
        match value {
            Resolved::Present(value) => Ok(Some(value)),
            Resolved::Absent(reason) if slot.absent == Absent::Reject => {
                Err(format!("missing {} operand: {reason:?}", slot.field))
            }
            Resolved::Absent(_) => Ok(None),
        }
    }

    fn evaluated_players(
        &self,
        task: &Task,
        player: &PlayerExpr,
    ) -> Result<Resolved<Vec<PlayerId>>, String> {
        let frame = self.kernel_root()?;
        let entry = self.entry(frame.stack_id)?;
        let players = match player {
            PlayerExpr::Is {
                who: Ref::EventPlayer { .. },
            } => {
                let Some(player) = entry.kind.event_binding()?.player else {
                    return Ok(Resolved::Absent(AbsentReason::NoEventMember));
                };
                vec![player]
            }
            PlayerExpr::Is {
                who: Ref::Target { slot },
            } => {
                let Some(&player) = self.bound_player_target(entry, *slot as usize) else {
                    return Ok(Resolved::Absent(AbsentReason::NotChosen));
                };
                if frame.legal_targets.get(*slot as usize) != Some(&true) {
                    return Ok(Resolved::Absent(AbsentReason::IllegalTarget));
                }
                vec![player]
            }
            PlayerExpr::Is {
                who: Ref::StepResult { step, .. },
            } => {
                let Some(result) = self.read_step_result(task, *step)? else {
                    return Ok(Resolved::Absent(AbsentReason::NotPerformed));
                };
                result.players.clone()
            }
            PlayerExpr::ControllerOf { object } | PlayerExpr::OwnerOf { object } => {
                let owner = matches!(player, PlayerExpr::OwnerOf { .. });
                if let Ref::Target { slot } = object {
                    if frame.legal_targets.get(*slot as usize) != Some(&true) {
                        return Ok(Resolved::Absent(AbsentReason::IllegalTarget));
                    }
                    vec![if owner {
                        let id = self
                            .bound_object_target(entry, *slot as usize)
                            .ok_or("missing target object")?;
                        self.state.rules.cards[&id.card].owner
                    } else {
                        self.kernel_target_facts(frame, *slot)?.controller()
                    }]
                } else {
                    let Some(facts) = self.measured_object(task, object)? else {
                        return Ok(Resolved::Absent(AbsentReason::NoEventMember));
                    };
                    vec![if owner { facts.owner } else { facts.controller }]
                }
            }
            _ => self.control_players(task, player)?,
        };
        if players.is_empty() {
            return Ok(Resolved::Present(players));
        }
        let players: Vec<_> = players.into_iter().filter(|p| self.is_live(*p)).collect();
        if players.is_empty() {
            Ok(Resolved::Absent(AbsentReason::DepartedPlayer))
        } else {
            Ok(Resolved::Present(players))
        }
    }

    fn measured_object(&self, task: &Task, object: &Ref) -> Result<Option<ObjectBefore>, String> {
        let entry = self.entry(self.kernel_root()?.stack_id)?;
        match object {
            Ref::Source => self.source_facts(entry).map(Some),
            // This is the occurrence's immutable subject, including death
            // look-back facts (CR 603.10a), never its new incarnation.
            Ref::EventObject { .. } => Ok(entry.kind.event_binding()?.object.as_deref().cloned()),
            Ref::BoundVar { id } => {
                let Some(Binding::Objects(ids)) = task.bindings.get(id) else {
                    return Err("missing measured binding".into());
                };
                let [id] = ids.as_slice() else {
                    return Err("expected one measured object".into());
                };
                self.current(*id)
                    .ok()
                    .map(|_| self.object_before(*id))
                    .transpose()
            }
            _ => Err("unsupported measured object".into()),
        }
    }

    fn evaluated_amount(&self, task: &Task, amount: &Amount) -> Result<Resolved<Quantity>, String> {
        let frame = self.kernel_root()?;
        let value = match amount {
            Amount::Var {
                of:
                    Ref::StepResult {
                        step,
                        role: StepRole::Amount,
                    },
            } => {
                let Some(amount) = self
                    .read_step_result(task, *step)?
                    .and_then(|result| result.amount.clone())
                else {
                    return Ok(Resolved::Absent(AbsentReason::NotPerformed));
                };
                amount
            }
            Amount::StatOf {
                object: Ref::Target { slot },
                stat,
            } => {
                if frame.legal_targets.get(*slot as usize) != Some(&true) {
                    return Ok(Resolved::Absent(AbsentReason::IllegalTarget));
                }
                let facts = self.kernel_target_facts(frame, *slot)?;
                match stat {
                    Stat::ManaValue => facts.mana_value().clone(),
                    Stat::Power => facts.power().cloned().unwrap_or_default(),
                    Stat::Toughness => facts.toughness().cloned().unwrap_or_default(),
                }
            }
            Amount::StatOf { object, stat } => {
                let Some(facts) = self.measured_object(task, object)? else {
                    return Ok(Resolved::Absent(AbsentReason::NoEventMember));
                };
                match stat {
                    Stat::Power => facts.characteristics.power.unwrap_or_default(),
                    Stat::Toughness => facts.characteristics.toughness.unwrap_or_default(),
                    Stat::ManaValue => facts.characteristics.printed_mana_value(),
                }
            }
            Amount::Count { set }
                if !matches!(
                    set,
                    SetExpr::Matching {
                        zone: ZoneExpr::Hand { .. } | ZoneExpr::Graveyard { .. },
                        ..
                    }
                ) =>
            {
                Quantity::from(self.control_objects(task, set)?.len() as i64)
            }
            Amount::LifeOf { player } => {
                let Resolved::Present(players) = self.evaluated_players(task, player)? else {
                    return Ok(Resolved::Absent(AbsentReason::DepartedPlayer));
                };
                let [player] = players.as_slice() else {
                    return Err("life measure requires one player".into());
                };
                self.state.rules.players[player.0 as usize].life.clone()
            }
            _ => self.kernel_amount(frame, amount)?,
        };
        Ok(Resolved::Present(value))
    }

    pub(super) fn evaluated_performable(&self, task: &Task, verb: &Verb) -> Result<bool, String> {
        let (_, player, amount) = verb.evaluated_operands().ok_or("missing verb signature")?;
        Ok(
            matches!(self.evaluated_players(task, player)?, Resolved::Present(players) if !players.is_empty())
                && matches!(self.evaluated_amount(task, amount)?, Resolved::Present(amount) if amount > Quantity::zero()),
        )
    }

    pub(super) fn evaluate_step(
        &mut self,
        mut task: Task,
        verb: &Verb,
        block: u32,
    ) -> Result<(), String> {
        let (signature, player, amount) =
            verb.evaluated_operands().ok_or("missing verb signature")?;
        let [recipient, quantity] = signature.slots else {
            return Err("invalid evaluated signature".into());
        };
        let empty_outcome = match signature.verb {
            EvaluatedVerb::Draw => InstructionOutcome::Effect { actual: None },
            _ => InstructionOutcome::Life { actual: None },
        };
        let Some(players) =
            Self::present_operand(recipient, self.evaluated_players(&task, player)?)?
        else {
            return self.complete_evaluated(&task, None, [empty_outcome]);
        };
        let Some(amount) = Self::present_operand(quantity, self.evaluated_amount(&task, amount)?)?
        else {
            return self.complete_evaluated(&task, None, [empty_outcome]);
        };
        // CR 107.1b: read a signed statistic and apply the quantity rule at use.
        let amount = match quantity.quantity {
            Some(QuantityDomain::NonNegative) => amount.max(Quantity::zero()),
            _ => return Err("invalid evaluated quantity policy".into()),
        };
        let result = StepResult {
            amount: Some(Quantity::zero()),
            ..StepResult::default()
        };
        if amount == Quantity::zero() || players.is_empty() {
            return self.complete_evaluated(&task, Some(result), [empty_outcome]);
        }
        match signature.distribution {
            Distribution::EachSequential => {
                task.stage = Stage::Step(StepProgress::Sequence {
                    players: players
                        .into_iter()
                        .map(|player| PlayerDrawAttempts {
                            player,
                            attempted: Quantity::zero(),
                            failed: Quantity::zero(),
                        })
                        .collect(),
                    at: 0,
                    remaining: amount.clone(),
                    total: amount,
                    result,
                    last: None,
                });
                self.step_sequence(task)
            }
            Distribution::EachSimultaneous => {
                let proposed = players
                    .into_iter()
                    .map(|player| match signature.verb {
                        EvaluatedVerb::GainLife => ProposedEvent::GainLife {
                            player,
                            amount: amount.clone(),
                        },
                        _ => ProposedEvent::LoseLife {
                            player,
                            amount: amount.clone(),
                        },
                    })
                    .collect::<Vec<_>>();
                let group = match proposed.as_slice() {
                    [only] => EventGroup::Occurrence {
                        proposed: only.clone(),
                    },
                    _ => EventGroup::Simultaneous {
                        occurrences: proposed,
                    },
                };
                let cause = self.instruction_cause(self.kernel_root()?, block)?;
                let batch = self.commit_batch(cause, group)?;
                let mut result = result;
                let mut outcomes = Vec::new();
                for event in batch.occurrences {
                    result.events.push(event.id);
                    match &event.actual {
                        EventOutcome::LifeGained { player, amount, .. }
                        | EventOutcome::LifeLost { player, amount, .. } => {
                            result.players.push(*player);
                            result.amount.as_mut().expect("life projection").add(amount);
                        }
                        EventOutcome::LifeChangeProhibited { .. } => {}
                        _ => return Err("unexpected life change receipt".into()),
                    }
                    outcomes.push(InstructionOutcome::Life {
                        actual: Some(event.actual),
                    });
                }
                self.complete_evaluated(&task, Some(result), outcomes)
            }
            _ => Err("unsupported evaluated distribution".into()),
        }
    }

    fn draw_player(task: &Task) -> Result<PlayerId, String> {
        let Stage::Step(StepProgress::Sequence { players, at, .. }) = &task.stage else {
            return Err("missing draw sequence".into());
        };
        players
            .get(*at)
            .map(|p| p.player)
            .ok_or("missing draw player".into())
    }

    pub(super) fn step_sequence(&mut self, task: Task) -> Result<(), String> {
        let player = Self::draw_player(&task)?;
        if !self.is_live(player) {
            return self.draw_result(task, None, None, None, false);
        }
        let object = self
            .top_library_card(player)
            .and_then(|card| self.object_id(card));
        if let Some(object) = object
            && self.state.rules.commanders.history(object.card).is_some()
        {
            let resume = self.save_frame(Frame::Kernel {
                task: Box::new(task),
            });
            self.rules_mut().frame = Frame::CommanderReturn {
                object,
                destination: Zone::Hand { owner: player },
                continuation: CommanderContinuation::ResolutionDraw { resume },
            };
            return Ok(());
        }
        match object {
            Some(object) => self.commit_draw(task, Some((object, Zone::Hand { owner: player }))),
            None => self.commit_draw(task, None),
        }
    }

    pub(in crate::engine) fn saved_draw_player(&self, link: FrameLink) -> Result<PlayerId, String> {
        let Frame::Kernel { task } = self.saved_frame(link)? else {
            return Err("missing saved draw sequence".into());
        };
        Self::draw_player(task)
    }

    pub(in crate::engine) fn finish_commander_draw(
        &mut self,
        link: FrameLink,
        object: ObjectId,
        destination: Option<Zone>,
    ) -> Result<(), String> {
        let Frame::Kernel { task } = self.saved_frame(link)?.clone() else {
            return Err("missing saved draw sequence".into());
        };
        match destination {
            Some(destination) => self.commit_draw(*task, Some((object, destination))),
            None => self.draw_result(*task, None, None, None, false),
        }
    }

    // CR 121.2, 121.4: one batch per attempt. The caller also persists its
    // projection and advances the sequence within this same RulesTxn.
    fn commit_draw(
        &mut self,
        task: Task,
        movement: Option<(ObjectId, Zone)>,
    ) -> Result<(), String> {
        let player = Self::draw_player(&task)?;
        let step = self.step_instance(&task)?;
        let (_, source_block) = self.node_at(&task)?;
        let cause = BatchCause::InstructionDraw {
            stack_id: step.stack,
            source: self.entry(step.stack)?.source,
            cursor: step.instruction,
            player,
            source_block,
        };
        let (proposed, controller_before) = match movement {
            Some((object, destination)) => (
                ProposedEvent::Move {
                    object,
                    destination,
                },
                Some(self.current(object)?.controller),
            ),
            None => (ProposedEvent::FailedDraw { player }, None),
        };
        let batch = self.commit_batch(cause, EventGroup::Occurrence { proposed })?;
        let [receipt] = batch.occurrences.as_slice() else {
            return Err("unexpected draw receipt count".into());
        };
        let movement = match &receipt.actual {
            EventOutcome::Moved { movement } => Some(movement.clone()),
            _ if movement.is_none() => None,
            _ => return Err("unexpected draw movement receipt".into()),
        };
        self.draw_result(
            task,
            Some(receipt.id.clone()),
            movement,
            controller_before,
            controller_before.is_none(),
        )
    }

    fn draw_result(
        &mut self,
        mut task: Task,
        event: Option<OccurrenceId>,
        movement: Option<MoveOutcome>,
        controller_before: Option<PlayerId>,
        failed_attempt: bool,
    ) -> Result<(), String> {
        let Stage::Step(StepProgress::Sequence {
            players,
            at,
            total,
            remaining,
            result,
            last,
        }) = &mut task.stage
        else {
            return Err("missing draw progress".into());
        };
        let player = players[*at].player;
        let one = Quantity::from(1);
        if let Some(event) = event {
            players[*at].attempted.add(&one);
            remaining.subtract(&one);
            result.events.push(event);
            if failed_attempt {
                players[*at].failed.add(&one);
            }
            if let Some(movement) = &movement
                && movement.destination == (Zone::Hand { owner: player })
            {
                result.amount.as_mut().expect("draw projection").add(&one);
                result.objects.push(ResultObject {
                    prior: movement.previous,
                    successor: Some(movement.successor),
                    controller_before: controller_before.ok_or("missing draw prior controller")?,
                });
                if !result.players.contains(&player) {
                    result.players.push(player);
                }
            }
            *last = Some(InstructionOutcome::Drawn {
                player,
                movement,
                failed: failed_attempt,
            });
        } else {
            *remaining = Quantity::zero();
            *last = Some(InstructionOutcome::PlayerActionSkipped { player });
        }
        if *remaining == Quantity::zero() {
            *at += 1;
            if *at == players.len() {
                let (verb, _) = self.node_at(&task)?;
                let Some((_, player, amount)) = verb.verb().and_then(Verb::evaluated_operands)
                else {
                    return Err("missing admitted draw".into());
                };
                let Stage::Step(StepProgress::Sequence {
                    players,
                    result,
                    last,
                    ..
                }) = task.stage.clone()
                else {
                    unreachable!("completed sequence");
                };
                let outcome = if *player == PlayerExpr::EachPlayer {
                    InstructionOutcome::EachPlayerDrawAttempts { players }
                } else if matches!(player, PlayerExpr::Is { .. }) && amount.as_fixed() == Some(&one)
                {
                    last.ok_or("missing single draw outcome")?
                } else if let [row] = players.as_slice() {
                    InstructionOutcome::DrawAttempts {
                        player: row.player,
                        attempted: row.attempted.clone(),
                        failed: row.failed.clone(),
                    }
                } else {
                    InstructionOutcome::EachPlayerDrawAttempts { players }
                };
                return self.complete_evaluated(&task, Some(result), [outcome]);
            }
            *remaining = total.clone();
        }
        self.install_kernel(task);
        Ok(())
    }

    pub(super) fn validate_sequence(
        &self,
        task: &Task,
        progress: &StepProgress,
        verb: &Verb,
    ) -> Result<(), String> {
        let StepProgress::Sequence {
            players,
            at,
            total,
            remaining,
            result,
            last,
        } = progress;
        let (_, operand, amount) = verb
            .evaluated_operands()
            .ok_or("missing sequence signature")?;
        let row = players.get(*at).ok_or("invalid draw sequence position")?;
        let mut total_at = row.attempted.clone();
        total_at.add(remaining);
        let mut attempts = Quantity::zero();
        let mut unique = BTreeSet::new();
        for (index, row) in players.iter().enumerate() {
            if row.attempted.is_negative()
                || row.failed.is_negative()
                || row.failed > row.attempted
                || row.attempted > *total
                || !unique.insert(row.player)
                || !self.state.rules.original_seating.contains(&row.player)
                || (index < *at && self.is_live(row.player) && row.attempted != *total)
                || (index > *at && row.attempted != Quantity::zero())
            {
                return Err("invalid draw sequence prefix".into());
            }
            attempts.add(&row.attempted);
        }
        if total_at != *total
            || *remaining <= Quantity::zero()
            || amount.as_fixed().is_some_and(|fixed| fixed != total)
            || (amount.is_announced_x()
                && self.entry(self.kernel_root()?.stack_id)?.x.as_ref() != Some(total))
            || attempts != Quantity::from(result.events.len() as i64)
            || (last.is_none() != (attempts == Quantity::zero() && *at == 0))
            || result.amount.as_ref() != Some(&Quantity::from(result.objects.len() as i64))
        {
            return Err("invalid draw sequence progress".into());
        }
        if let Resolved::Present(expected) = self.evaluated_players(task, operand)?
            && expected
                != players
                    .iter()
                    .filter(|p| self.is_live(p.player))
                    .map(|p| p.player)
                    .collect::<Vec<_>>()
        {
            return Err("invalid draw sequence recipient".into());
        }
        let (completed, first_batch) = self.step_collector()?;
        let last_completed = completed
            .0
            .values()
            .filter_map(|prior| prior.events.last().map(|event| event.batch))
            .max();
        if result.events.iter().any(|event| !event.path.is_empty())
            || result
                .events
                .first()
                .zip(last_completed)
                .is_some_and(|(first, last)| first.batch <= last)
        {
            return Err("draw sequence events overlap completed work".into());
        }
        self.validate_result(result, first_batch)
    }

    fn step_order(&self, task: &Task) -> Result<Vec<(u32, u32)>, String> {
        let mut prefix = task.clone();
        prefix.path.clear();
        let mut iterations = task.iterations.iter().copied();
        let mut order = Vec::new();
        for &index in &task.path {
            let iteration = match self.node_at(&prefix)?.0 {
                Node::ForEach { over, .. } => {
                    let ordinal = iterations.next().ok_or("missing step iteration")?;
                    let limit = match over {
                        Domain::Players { .. } => self.state.rules.original_seating.len(),
                        Domain::Objects { .. } => self.state.rules.cards.len(),
                    };
                    if ordinal as usize >= limit {
                        return Err("invalid step iteration ordinal".into());
                    }
                    ordinal
                }
                _ => 0,
            };
            order.push((index, iteration));
            prefix.path.push(index);
        }
        if iterations.next().is_some() {
            return Err("unexpected step iteration ordinal".into());
        }
        Ok(order)
    }

    pub(super) fn validate_step_address(&self, task: &Task) -> Result<(), String> {
        self.step_order(task)?;
        let mut scope = BTreeMap::new();
        let mut prefix = task.clone();
        prefix.path.clear();
        for &index in &task.path {
            match self.node_at(&prefix)?.0 {
                Node::ForEach { var, over, .. } => {
                    scope.insert(*var, (matches!(over, Domain::Players { .. }), true));
                }
                Node::Choose { var, .. } => {
                    scope.insert(*var, (false, false));
                }
                _ => {}
            }
            prefix.path.push(index);
        }
        if scope.len() != task.bindings.len() {
            return Err("unexpected lexical step bindings".into());
        }
        for (var, (player, singleton)) in scope {
            let valid = match task.bindings.get(&var) {
                Some(Binding::Player(player_id)) => {
                    player && self.state.rules.original_seating.contains(player_id)
                }
                Some(Binding::Objects(objects)) => {
                    !player
                        && (!singleton || objects.len() == 1)
                        && objects.iter().collect::<BTreeSet<_>>().len() == objects.len()
                }
                None => false,
            };
            if !valid {
                return Err("invalid lexical step binding".into());
            }
        }
        Ok(())
    }

    pub(super) fn validate_step_results(&self, root: &Task) -> Result<(), String> {
        let Stage::Complete {
            results,
            first_batch,
            ..
        } = &root.stage
        else {
            return Err("invalid result collector".into());
        };
        if *first_batch > self.state.rules.next_batch_id {
            return Err("invalid result batch boundary".into());
        }
        let mut events = BTreeSet::new();
        let active = match &self.state.rules.frame {
            Frame::Kernel { task } => Some(task.as_ref()),
            Frame::CommanderReturn {
                continuation: CommanderContinuation::ResolutionDraw { resume },
                ..
            } => match self.saved_frame(*resume)? {
                Frame::Kernel { task } => Some(task.as_ref()),
                _ => return Err("missing saved draw sequence".into()),
            },
            _ => None,
        };
        self.validate_saved_step_results(root, results, *first_batch, &mut events, active)
    }

    fn validate_saved_step_results(
        &self,
        root: &Task,
        results: &StepResults,
        first_batch: u64,
        events: &mut BTreeSet<(u64, Vec<u32>)>,
        active: Option<&Task>,
    ) -> Result<(), String> {
        for ((path, iterations), result) in &results.0 {
            let mut address = root.clone();
            address.path.clone_from(path);
            address.iterations.clone_from(iterations);
            let node = self.node_at(&address)?.0;
            let (signature, _, _) = node
                .verb()
                .and_then(Verb::evaluated_operands)
                .ok_or("result does not name an evaluated step")?;
            let order = self.step_order(&address)?;
            if let Some(active) = active
                && order >= self.step_order(active)?
            {
                return Err("step result does not precede current work".into());
            }
            if signature.verb != EvaluatedVerb::Draw && !result.objects.is_empty() {
                return Err("life result has objects".into());
            }
            if result
                .events
                .iter()
                .any(|id| !events.insert((id.batch, id.path.clone())))
            {
                return Err("duplicate step event".into());
            }
            self.validate_result(result, first_batch)?;
        }
        Ok(())
    }

    fn validate_result(&self, result: &StepResult, first_batch: u64) -> Result<(), String> {
        let rules = &self.state.rules;
        if result
            .events
            .iter()
            .any(|event| event.batch < first_batch || event.batch >= rules.next_batch_id)
            || !result
                .events
                .windows(2)
                .all(|pair| (&pair[0].batch, &pair[0].path) < (&pair[1].batch, &pair[1].path))
            || result.amount.as_ref().is_none_or(Quantity::is_negative)
            || result.players.iter().collect::<BTreeSet<_>>().len() != result.players.len()
            || result
                .players
                .iter()
                .any(|p| !rules.original_seating.contains(p))
            || result.objects.iter().any(|object| {
                !rules.original_seating.contains(&object.controller_before)
                    || object.successor.is_none_or(|next| {
                        next.card != object.prior.card
                            || object.prior.incarnation.checked_add(1) != Some(next.incarnation)
                    })
                    || !rules.cards.contains_key(&object.prior.card)
            })
        {
            return Err("invalid evaluated step result".into());
        }
        Ok(())
    }
}
