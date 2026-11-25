//! A suspended node names its location in the admitted instruction. Sequences
//! and iterations save their remaining work on the same stack as rule frames.
//! No snapshot supplies an executable node.
use super::*;
use mtg_contracts::kir::{Cmp, Cond, Domain, Payment};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Binding {
    Player(PlayerId),
    Objects(Vec<ObjectId>),
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "stage", rename_all = "snake_case", deny_unknown_fields)]
enum Stage {
    #[default]
    Run,
    Leaf,
    Complete {
        outcomes: Vec<InstructionOutcome>,
    },
    Optional {
        chooser: PlayerId,
    },
    Life {
        payer: PlayerId,
        amount: Quantity,
    },
    Choose {
        chooser: PlayerId,
        remaining: Vec<ObjectId>,
        selected: Vec<ObjectId>,
        minimum: usize,
        maximum: usize,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Task {
    pub(in crate::engine) resume: FrameLink,
    path: Vec<usize>,
    bindings: BTreeMap<u32, Binding>,
    stage: Stage,
}

impl Task {
    fn child(&self, index: usize) -> Self {
        let mut path = self.path.clone();
        path.push(index);
        Self {
            path,
            bindings: self.bindings.clone(),
            resume: self.resume,
            stage: Stage::Run,
        }
    }

    pub(in crate::engine) fn is_leaf(&self) -> bool {
        self.stage == Stage::Leaf
    }

    pub(in crate::engine) fn candidate(&self) -> Option<ObjectId> {
        if let Stage::Choose { remaining, .. } = &self.stage {
            remaining.last().copied()
        } else {
            None
        }
    }

    fn chooser(&self) -> Option<PlayerId> {
        match self.stage {
            Stage::Run | Stage::Leaf | Stage::Complete { .. } => None,
            Stage::Optional { chooser } | Stage::Choose { chooser, .. } => Some(chooser),
            Stage::Life { payer, .. } => Some(payer),
        }
    }
}

impl RulesTxn {
    pub(super) fn begin_kernel_control(&mut self, frame: Resolving) -> Result<(), String> {
        let resume = self.save_frame(Frame::Resolve { frame });
        let root = Task {
            resume,
            path: Vec::new(),
            bindings: BTreeMap::new(),
            stage: Stage::Complete {
                outcomes: Vec::new(),
            },
        };
        let mut task = Task {
            stage: Stage::Run,
            ..root.clone()
        };
        task.resume = self.save_frame(Frame::Kernel {
            task: Box::new(root),
        });
        self.install_kernel(task);
        Ok(())
    }

    fn install_kernel(&mut self, task: Task) {
        self.rules_mut().frame = Frame::Kernel {
            task: Box::new(task),
        };
    }

    pub(in crate::engine) fn kernel_root(&self) -> Result<&Resolving, String> {
        match self.state.rules.frames.first() {
            Some(Frame::Resolve { frame }) => Ok(frame),
            _ => Err("missing kernel invocation".into()),
        }
    }

    fn node_at(&self, task: &Task) -> Result<(&Node, u32), String> {
        let frame = self.kernel_root()?;
        let Some(Instruction::Kernel { node, source_block }) = self
            .original_program_for(self.entry(frame.stack_id)?)?
            .instructions
            .get(frame.cursor as usize)
        else {
            return Err("missing kernel instruction".into());
        };
        let mut node = node;
        for index in &task.path {
            node = match node {
                Node::Seq(nodes) => nodes.get(*index),
                Node::If {
                    then, otherwise, ..
                } => match index {
                    0 => Some(then.as_ref()),
                    1 => otherwise.as_deref(),
                    _ => None,
                },
                Node::ForEach { body, .. }
                | Node::Choose { body, .. }
                | Node::Optional { node: body, .. }
                | Node::UnlessPays { node: body, .. }
                | Node::IfPays { node: body, .. }
                    if *index == 0 =>
                {
                    Some(body.as_ref())
                }
                _ => None,
            }
            .ok_or("invalid kernel continuation address")?;
        }
        Ok((node, *source_block))
    }

    fn control_objects(&self, task: &Task, set: &SetExpr) -> Result<Vec<ObjectId>, String> {
        let frame = self.kernel_root()?;
        let entry = self.entry(frame.stack_id)?;
        Ok(match set {
            SetExpr::Matching { zone, filter } => self.kernel_set(frame, zone, filter)?.0,
            SetExpr::One {
                object: Ref::Source,
            } => vec![entry.source],
            SetExpr::One {
                object: Ref::Target { slot },
            } => {
                let slot = *slot as usize;
                if frame.legal_targets.get(slot) == Some(&true) {
                    self.bound_object_target(entry, slot)
                        .copied()
                        .into_iter()
                        .collect()
                } else {
                    Vec::new()
                }
            }
            SetExpr::One {
                object: Ref::BoundVar { id },
            }
            | SetExpr::Pool {
                pool: Ref::BoundVar { id },
            } => {
                let Some(Binding::Objects(objects)) = task.bindings.get(id) else {
                    return Err("missing kernel object binding".into());
                };
                objects.clone()
            }
            _ => return Err("unsupported kernel object expression".into()),
        })
    }

    fn control_players(
        &self,
        task: &Task,
        expression: &PlayerExpr,
    ) -> Result<Vec<PlayerId>, String> {
        let frame = self.kernel_root()?;
        Ok(match expression {
            PlayerExpr::EachPlayer => self.apnap_players(),
            PlayerExpr::Is {
                who: Ref::BoundVar { id },
            } => {
                let Some(Binding::Player(p)) = task.bindings.get(id) else {
                    return Err("missing kernel player binding".into());
                };
                vec![*p]
            }
            // CR 608.2h: a filtered target's current or last-known
            // controller, captured by the resolving frame.
            PlayerExpr::ControllerOf {
                object: Ref::Target { .. },
            } => match self.kernel_players(frame, expression)? {
                Players::One(player) => vec![player],
                Players::Nobody => vec![],
                Players::EachOpponent => return Err("invalid target controller".into()),
            },
            PlayerExpr::OwnerOf { object } | PlayerExpr::ControllerOf { object } => {
                let objects = self.control_objects(task, &SetExpr::One { object: *object })?;
                let [id] = objects.as_slice() else {
                    return Err("kernel reference requires one object".into());
                };
                vec![if matches!(expression, PlayerExpr::OwnerOf { .. }) {
                    self.state
                        .rules
                        .cards
                        .get(&id.card)
                        .ok_or("missing kernel owner")?
                        .owner
                } else {
                    self.current(*id)?.controller
                }]
            }
            _ => match self.kernel_players(frame, expression)? {
                Players::Nobody => vec![],
                Players::One(p) => vec![p],
                Players::EachOpponent => {
                    let controller = self.entry(frame.stack_id)?.controller;
                    self.apnap_seats()
                        .filter(|p| *p != controller && self.is_live(*p))
                        .collect()
                }
            },
        })
    }

    fn chooser(&self, task: &Task, expression: &PlayerExpr) -> Result<Option<PlayerId>, String> {
        match self.control_players(task, expression)?.as_slice() {
            [] => Ok(None),
            [player] => Ok(self.is_live(*player).then_some(*player)),
            _ => Err("kernel choice requires one player".into()),
        }
    }

    fn control_amount(&self, task: &Task, amount: &Amount) -> Result<Quantity, String> {
        match amount {
            Amount::LifeOf { player } => Ok(self.state.rules.players[self
                .chooser(task, player)?
                .ok_or("missing kernel life subject")?
                .0 as usize]
                .life
                .clone()),
            Amount::Count { set } => {
                Ok(Quantity::from(self.control_objects(task, set)?.len() as i64))
            }
            _ => self.kernel_amount(self.kernel_root()?, amount),
        }
    }

    fn condition(&self, task: &Task, cond: &Cond) -> Result<bool, String> {
        Ok(match cond {
            Cond::Compare { left, cmp, right } => {
                let left = self.control_amount(task, left)?;
                let right = self.control_amount(task, right)?;
                match cmp {
                    Cmp::Exactly => left == right,
                    Cmp::AtLeast => left >= right,
                    Cmp::AtMost => left <= right,
                }
            }
            Cond::Exists { set } => !self.control_objects(task, set)?.is_empty(),
            Cond::ActivePlayer { .. } => {
                return Err("active-player condition outside statics".into());
            }
            Cond::Satisfies { .. } | Cond::CastBy { .. } => {
                return Err("intervening condition in a resolving node".into());
            }
            Cond::CostPaid { identity } => self
                .entry(self.kernel_root()?.stack_id)?
                .selected_additional_costs
                .as_ref()
                .is_some_and(|paid| paid.contains(identity)),
            Cond::Not { condition } => !self.condition(task, condition)?,
            Cond::All { conditions } => {
                for cond in conditions {
                    if !self.condition(task, cond)? {
                        return Ok(false);
                    }
                }
                true
            }
        })
    }

    fn return_kernel(&mut self, resume: FrameLink) -> Result<(), String> {
        let Frame::Kernel { task } = self.saved_frame(resume)?.clone() else {
            return Err("invalid kernel return frame".into());
        };
        if let Stage::Complete { mut outcomes } = task.stage {
            let outcome = match outcomes.len() {
                0 => InstructionOutcome::Effect { actual: None },
                1 => outcomes.remove(0),
                _ => InstructionOutcome::Kernel { outcomes },
            };
            self.finish_instruction(self.kernel_root()?.clone(), outcome)
        } else {
            self.install_kernel(*task);
            Ok(())
        }
    }

    /// The saved resolving invocation is followed by the task that collects
    /// this instruction's leaf results in execution order, across suspensions.
    fn kernel_results(&mut self) -> Result<&mut Vec<InstructionOutcome>, String> {
        let Some(Frame::Kernel { task }) = self.rules_mut().frames.get_mut(1) else {
            return Err("missing kernel completion".into());
        };
        let Stage::Complete { outcomes } = &mut task.stage else {
            return Err("missing kernel result collector".into());
        };
        Ok(outcomes)
    }

    fn schedule(
        &mut self,
        resume: FrameLink,
        tasks: impl DoubleEndedIterator<Item = Task>,
    ) -> Result<(), String> {
        let mut tasks = tasks.rev().peekable();
        let mut parent = resume;
        while let Some(mut task) = tasks.next() {
            task.resume = parent;
            if tasks.peek().is_none() {
                self.install_kernel(task);
                return Ok(());
            }
            parent = self.save_frame(Frame::Kernel {
                task: Box::new(task),
            });
        }
        self.return_kernel(parent)
    }

    pub(in crate::engine) fn kernel_request(
        &self,
        task: &Task,
    ) -> Result<Option<Decision<Rules>>, String> {
        let Some(player) = task.chooser().filter(|p| self.is_live(*p)) else {
            return Ok(None);
        };
        if !self.still_offered(task)? {
            return Ok(None);
        }
        Ok(Some(self.pick_flags(
            player,
            Reason::OptionalEffect,
            &[Flag::Accept, Flag::Decline],
        )))
    }

    /// CR 608.2d, 800.4a: a suspended optional choice is offered only while
    /// accepting it is still possible. A departure while it waits (the
    /// affected player conceding, a target leaving) withdraws the offer, as
    /// a departed chooser does; the continuation then skips the node.
    fn still_offered(&self, task: &Task) -> Result<bool, String> {
        match (&task.stage, self.node_at(task)?.0) {
            (Stage::Optional { .. }, Node::Optional { node, .. }) => self.performable(task, node),
            _ => Ok(true),
        }
    }

    pub(in crate::engine) fn kernel_choice_context(
        &self,
        task: &Task,
    ) -> Result<(u32, Option<ObjectId>), String> {
        Ok((self.node_at(task)?.1, task.candidate()))
    }

    pub(in crate::engine) fn kernel_charge(&self) -> usize {
        let charge = |frame: &Frame| match frame {
            Frame::Kernel { task } => stored_history::payload_charge(task),
            _ => 0,
        };
        self.state
            .rules
            .frames
            .iter()
            .fold(charge(&self.state.rules.frame), |sum, f| {
                sum.saturating_add(charge(f))
            })
    }

    pub(in crate::engine) fn validate_kernel_task(
        &self,
        task: &Task,
        depth: usize,
    ) -> Result<(), String> {
        let (node, _) = self.node_at(task)?;
        let root = self.kernel_root()?;
        self.validate_resolving(root)?;
        if root
            .condition_check
            .as_ref()
            .is_some_and(|check| !check.passed())
        {
            return Err("snapshot kernel task after failed intervening condition".into());
        }
        if task.chooser().is_some_and(|p| p.0 >= 4) {
            return Err("invalid kernel chooser".into());
        }
        if !matches!(task.stage, Stage::Complete { .. })
            && !matches!(self.state.rules.frames.get(1), Some(Frame::Kernel { task }) if matches!(task.stage, Stage::Complete { .. }))
        {
            return Err("missing kernel completion".into());
        }
        match (&task.stage, node) {
            (Stage::Complete { outcomes }, _)
                if task.resume == FrameLink(0)
                    && task.path.is_empty()
                    && task.bindings.is_empty()
                    && depth == 1
                    && depth < self.state.rules.frames.len() =>
            {
                for outcome in outcomes {
                    self.validate_library_reorder_outcome(self.kernel_root()?, None, outcome)?;
                }
                Ok(())
            }
            (Stage::Leaf, Node::Do(_)) | (Stage::Run, _) => Ok(()),
            // The saved chooser is the one the admitted node names.
            (Stage::Optional { chooser }, Node::Optional { chooser: named, .. }) => {
                if self.control_players(task, named)? != [*chooser] {
                    return Err("kernel chooser differs from its node".into());
                }
                Ok(())
            }
            // CR 118.12: the saved payer and amount are the admitted terms.
            (
                Stage::Life { payer, amount },
                Node::UnlessPays {
                    payer: named,
                    payment: Payment::Life { amount: admitted },
                    ..
                }
                | Node::IfPays {
                    payer: named,
                    payment: Payment::Life { amount: admitted },
                    ..
                },
            ) => {
                if self.control_players(task, named)? != [*payer] {
                    return Err("kernel payer differs from its node".into());
                }
                if self.control_amount(task, admitted)? != *amount
                    || !self.state.rules.players[payer.0 as usize].can_pay_life(amount)
                {
                    return Err("kernel life payment differs from its node".into());
                }
                Ok(())
            }
            (
                Stage::Choose {
                    remaining,
                    selected,
                    minimum,
                    maximum,
                    ..
                },
                Node::Choose { .. },
            ) if minimum <= maximum
                && !remaining.is_empty()
                && selected.len() < *maximum
                && selected.len() + remaining.len() > *minimum =>
            {
                Ok(())
            }
            _ => Err("invalid kernel suspension".into()),
        }
    }

    pub(in crate::engine) fn step_kernel_task(&mut self, mut task: Task) -> Result<(), String> {
        let (node, block) = self.node_at(&task)?;
        let node = node.clone();
        if let Some(player) = task.chooser() {
            let perform = if self.is_live(player) && self.still_offered(&task)? {
                let input = self
                    .rules_mut()
                    .pending
                    .take()
                    .ok_or("missing kernel decision")?;
                input.flag(Flag::Accept)?
            } else {
                self.rules_mut().pending = None;
                false
            };
            match std::mem::take(&mut task.stage) {
                Stage::Optional { .. } if perform => self.install_kernel(task.child(0)),
                Stage::Optional { .. } => return self.return_kernel(task.resume),
                // CR 118.12: "unless" performs the node when the payer
                // declines; "if they do" performs it once they have paid.
                Stage::Life { amount, .. } if perform => {
                    if !self.state.rules.players[player.0 as usize].can_pay_life(&amount) {
                        return Err("unpayable kernel life payment".into());
                    }
                    if amount > Quantity::zero() {
                        self.commit(
                            ProposedEvent::LoseLife { player, amount },
                            self.instruction_cause(self.kernel_root()?, block)?,
                        )?;
                    }
                    return self.paid(task, &node, true);
                }
                Stage::Life { .. } => return self.paid(task, &node, false),
                Stage::Choose {
                    mut remaining,
                    mut selected,
                    minimum,
                    maximum,
                    ..
                } => {
                    let object = remaining.pop().ok_or("missing kernel candidate")?;
                    if perform {
                        selected.push(object);
                    }
                    let Node::Choose { var, .. } = node else {
                        return Err("missing choose node".into());
                    };
                    self.choose_next(task, var, player, remaining, selected, minimum, maximum)?;
                }
                Stage::Run | Stage::Leaf | Stage::Complete { .. } => {
                    return Err("unexpected kernel input".into());
                }
            }
            return Ok(());
        }
        match &node {
            Node::Seq(nodes) => {
                self.schedule(task.resume, (0..nodes.len()).map(|i| task.child(i)))?
            }
            Node::If {
                condition,
                otherwise,
                ..
            } => {
                if self.condition(&task, condition)? {
                    self.install_kernel(task.child(0));
                } else if otherwise.is_some() {
                    self.install_kernel(task.child(1));
                } else {
                    self.return_kernel(task.resume)?;
                }
            }
            Node::Optional {
                chooser,
                node: child,
            } => {
                let Some(chooser) = self.chooser(&task, chooser)? else {
                    return self.return_kernel(task.resume);
                };
                // CR 608.2d, 701.26a: no choice is offered to perform an
                // action that cannot be performed.
                if !self.performable(&task, child)? {
                    return self.return_kernel(task.resume);
                }
                task.stage = Stage::Optional { chooser };
                self.install_kernel(task);
            }
            Node::ForEach { var, over, .. } => {
                let members = match over {
                    Domain::Players { players } => self
                        .control_players(&task, players)?
                        .into_iter()
                        .map(Binding::Player)
                        .collect::<Vec<_>>(),
                    Domain::Objects { set } => self
                        .control_objects(&task, set)?
                        .into_iter()
                        .map(|id| Binding::Objects(vec![id]))
                        .collect(),
                };
                self.schedule(
                    task.resume,
                    members.into_iter().map(|member| {
                        let mut child = task.child(0);
                        child.bindings.insert(*var, member);
                        child
                    }),
                )?;
            }
            Node::Choose {
                var,
                chooser,
                from,
                minimum,
                maximum,
                ..
            } => {
                let Some(player) = self.chooser(&task, chooser)? else {
                    return self.return_kernel(task.resume);
                };
                let mut objects = self.control_objects(&task, from)?;
                for object in &objects {
                    self.handle_for(player, *object)?;
                }
                let min = self.control_amount(&task, minimum)?;
                let max = self.control_amount(&task, maximum)?;
                if min.is_negative() || max < min {
                    return Err("invalid kernel choice bounds".into());
                }
                let bound =
                    |q: Quantity| (q.to_u32().unwrap_or(u32::MAX) as usize).min(objects.len());
                let (min, max) = (bound(min), bound(max));
                objects.reverse();
                self.choose_next(task, *var, player, objects, vec![], min, max)?;
            }
            Node::UnlessPays { payer, payment, .. } | Node::IfPays { payer, payment, .. } => {
                // CR 800.4a: a payer who has left the game pays nothing.
                let Some(player) = self.chooser(&task, payer)? else {
                    return self.paid(task, &node, false);
                };
                match payment {
                    Payment::Life { amount } => {
                        let amount = self.control_amount(&task, amount)?;
                        if amount.is_negative() {
                            return Err("negative kernel life payment".into());
                        }
                        // CR 119.4: a player can't pay more life than they have.
                        if self.state.rules.players[player.0 as usize].can_pay_life(&amount) {
                            task.stage = Stage::Life {
                                payer: player,
                                amount,
                            };
                            self.install_kernel(task);
                        } else {
                            self.paid(task, &node, false)?;
                        }
                    }
                    Payment::Generic { amount } => {
                        let amount = self.control_amount(&task, amount)?;
                        self.begin_kernel_payment(
                            task,
                            player,
                            ManaCost {
                                generic: amount,
                                colored: vec![],
                            },
                        )?;
                    }
                }
            }
            Node::Do(Verb::GainLife { player, amount } | Verb::LoseLife { player, amount }) => {
                let amount = self.control_amount(&task, amount)?;
                if amount > Quantity::zero() {
                    let occurrences = self
                        .control_players(&task, player)?
                        .into_iter()
                        .filter(|p| self.is_live(*p))
                        .map(|player| {
                            if matches!(node, Node::Do(Verb::GainLife { .. })) {
                                ProposedEvent::GainLife {
                                    player,
                                    amount: amount.clone(),
                                }
                            } else {
                                ProposedEvent::LoseLife {
                                    player,
                                    amount: amount.clone(),
                                }
                            }
                        })
                        .collect::<Vec<_>>();
                    if !occurrences.is_empty() {
                        let batch = self.commit_batch(
                            self.instruction_cause(self.kernel_root()?, block)?,
                            EventGroup::Simultaneous { occurrences },
                        )?;
                        self.kernel_results()?
                            .extend(batch.occurrences.into_iter().map(|event| {
                                InstructionOutcome::Life {
                                    actual: Some(event.actual),
                                }
                            }));
                    }
                }
                self.return_kernel(task.resume)?;
            }
            Node::Do(_) => self.begin_kernel_leaf(task)?,
            Node::Reflexive { payment, .. } => {
                let player = self.entry(self.kernel_root()?.stack_id)?.controller;
                if self.is_live(player) {
                    self.begin_kernel_payment(task, player, payment.clone())?;
                } else {
                    self.return_kernel(task.resume)?;
                }
            }
            Node::Delayed { .. } => {
                return Err("unsupported kernel trigger control".into());
            }
        }
        Ok(())
    }

    /// CR 608.2d: whether an optional body can be performed now, so that
    /// accepting it is a legal choice. A sequence is performable when any of
    /// its verbs is.
    /// * Tap, untap, counters: an object the verb names is on the
    ///   battlefield in a state it can change (CR 701.26a-b, 122.1).
    /// * Move: the object's incarnation still exists; a zone change makes a
    ///   new one (CR 400.7).
    /// * Draw, life: a live player and a positive amount. An empty library
    ///   does not make a draw impossible (CR 121.3, 608.2d).
    /// * Mill: a live player whose library holds the whole amount (CR 701.17b).
    fn performable(&self, task: &Task, node: &Node) -> Result<bool, String> {
        let present = |set: &SetExpr, wanted: &dyn Fn(&Object) -> bool| -> Result<bool, String> {
            Ok(self.control_objects(task, set)?.into_iter().any(|id| {
                self.current(id)
                    .is_ok_and(|object| object.present_on_battlefield() && wanted(object))
            }))
        };
        Ok(match node {
            Node::Seq(nodes) => {
                for node in nodes {
                    if self.performable(task, node)? {
                        return Ok(true);
                    }
                }
                false
            }
            Node::Do(Verb::Tap { what }) => present(what, &|o| !o.tapped)?,
            Node::Do(Verb::Untap { what }) => present(what, &|o| o.tapped)?,
            Node::Do(Verb::PutCounters { on, .. }) => present(on, &|_| true)?,
            Node::Do(Verb::Move { what, .. }) => self
                .control_objects(task, what)?
                .into_iter()
                .any(|id| self.current(id).is_ok()),
            Node::Do(
                Verb::Draw { player, amount }
                | Verb::GainLife { player, amount }
                | Verb::LoseLife { player, amount },
            ) => {
                self.control_amount(task, amount)? > Quantity::zero()
                    && self
                        .control_players(task, player)?
                        .into_iter()
                        .any(|p| self.is_live(p))
            }
            Node::Do(Verb::Mill { player, amount }) => {
                let amount = self.control_amount(task, amount)?;
                amount > Quantity::zero()
                    && self.control_players(task, player)?.into_iter().any(|p| {
                        let library = self
                            .state
                            .rules
                            .objects
                            .values()
                            .filter(|o| o.zone == Zone::Library { owner: p })
                            .count();
                        self.is_live(p) && Quantity::from(library as i64) >= amount
                    })
            }
            // Composition admits optional bodies of the verbs above only
            // (`compose::control::decided`); nested control decides its own
            // choices when it runs.
            _ => true,
        })
    }

    pub(in crate::engine) fn kernel_payment_terms(
        &self,
        resume: FrameLink,
    ) -> Result<(PlayerId, u32, ManaCost), String> {
        let Frame::Kernel { task } = self.saved_frame(resume)? else {
            return Err("missing kernel payment parent".into());
        };
        let (node, block) = self.node_at(task)?;
        match node {
            Node::Reflexive { payment, .. } => Ok((
                self.entry(self.kernel_root()?.stack_id)?.controller,
                block,
                payment.clone(),
            )),
            Node::UnlessPays {
                payer,
                payment: Payment::Generic { amount },
                ..
            }
            | Node::IfPays {
                payer,
                payment: Payment::Generic { amount },
                ..
            } => {
                let players = self.control_players(task, payer)?;
                let [player] = players.as_slice() else {
                    return Err("invalid kernel payer".into());
                };
                Ok((
                    *player,
                    block,
                    ManaCost {
                        generic: self.control_amount(task, amount)?,
                        colored: vec![],
                    },
                ))
            }
            _ => Err("invalid kernel payment parent".into()),
        }
    }

    pub(in crate::engine) fn finish_kernel_payment(
        &mut self,
        resume: FrameLink,
        payment: Option<&CommittedBatch>,
    ) -> Result<(), String> {
        let Frame::Kernel { task } = self.saved_frame(resume)?.clone() else {
            return Err("invalid kernel payment parent".into());
        };
        let node = self.node_at(&task)?.0.clone();
        if let Node::Reflexive { payment: cost, .. } = node {
            if let Some(batch) = payment {
                self.collect_reflexive_payment(&cost, batch)?;
            }
            return self.return_kernel(task.resume);
        }
        self.paid(*task, &node, payment.is_some())
    }

    /// CR 118.12, 118.12a: continue a payment node once the payer has paid
    /// or not: `UnlessPays` performs its node when unpaid, `IfPays` when paid.
    fn paid(&mut self, task: Task, node: &Node, paid: bool) -> Result<(), String> {
        let perform = match node {
            Node::UnlessPays { .. } => !paid,
            Node::IfPays { .. } => paid,
            _ => return Err("invalid kernel payment node".into()),
        };
        if perform {
            self.install_kernel(task.child(0));
            Ok(())
        } else {
            self.return_kernel(task.resume)
        }
    }

    fn choose_next(
        &mut self,
        mut task: Task,
        var: u32,
        chooser: PlayerId,
        remaining: Vec<ObjectId>,
        mut selected: Vec<ObjectId>,
        minimum: usize,
        maximum: usize,
    ) -> Result<(), String> {
        if remaining.is_empty()
            || selected.len() == maximum
            || remaining.len() + selected.len() == minimum
        {
            if remaining.len() + selected.len() == minimum {
                selected.extend(remaining.into_iter().rev());
            }
            let mut child = task.child(0);
            child.bindings.insert(var, Binding::Objects(selected));
            self.install_kernel(child);
        } else {
            task.stage = Stage::Choose {
                chooser,
                remaining,
                selected,
                minimum,
                maximum,
            };
            self.install_kernel(task);
        }
        Ok(())
    }
}

/// While a leaf is executing, ordinary verb executors see that leaf at the
/// root instruction's cursor. The executable program is always derived from
/// the admitted catalog. Only the marker and its address enter the snapshot.
pub struct LeafProgram {
    pub(in crate::engine) marker: FrameLink,
    pub(in crate::engine) invocation: StackId,
    pub(in crate::engine) program: SpellProgram,
}

impl RulesTxn {
    fn begin_kernel_leaf(&mut self, mut task: Task) -> Result<(), String> {
        task.stage = Stage::Leaf;
        self.save_frame(Frame::Kernel {
            task: Box::new(task),
        });
        self.rebuild_kernel_leaf()?;
        self.rules_mut().frame = Frame::Resolve {
            frame: self.kernel_root()?.clone(),
        };
        Ok(())
    }

    pub(in crate::engine) fn rebuild_kernel_leaf(&mut self) -> Result<(), String> {
        let Some((index, task)) = self
            .state
            .rules
            .frames
            .iter()
            .enumerate()
            .find_map(|(i, f)| match f {
                Frame::Kernel { task } if task.is_leaf() => Some((i, task)),
                _ => None,
            })
        else {
            self.kernel_leaf = None;
            return Ok(());
        };
        let (node, source_block) = self.node_at(task)?;
        if !matches!(node, Node::Do(_)) {
            return Err("kernel leaf must be a verb".into());
        }
        let frame = self.kernel_root()?;
        let mut program = self
            .original_program_for(self.entry(frame.stack_id)?)?
            .clone();
        program.instructions[frame.cursor as usize] = Instruction::Kernel {
            node: node.clone(),
            source_block,
        };
        self.kernel_leaf = Some(Arc::new(LeafProgram {
            marker: FrameLink(index),
            invocation: frame.stack_id,
            program,
        }));
        Ok(())
    }

    pub(in crate::engine) fn resume_finished_kernel_leaf(&mut self) -> Result<(), String> {
        let Some(leaf) = &self.kernel_leaf else {
            return Ok(());
        };
        let marker = leaf.marker;
        let root = self.kernel_root()?;
        if matches!(&self.state.rules.frame, Frame::Resolve {frame} if frame.stack_id == root.stack_id && frame.cursor > root.cursor)
        {
            let Frame::Kernel { task } = self.saved_frame(marker)? else {
                return Err("missing kernel leaf marker".into());
            };
            let resume = task.resume;
            let cursor = root.cursor;
            let Frame::Resolve { mut frame } = self.working_frame() else {
                return Err("missing completed kernel leaf".into());
            };
            if frame.cursor.checked_sub(1) != Some(cursor)
                || frame.outcomes.len() != frame.cursor as usize
            {
                return Err("invalid kernel leaf completion".into());
            }
            let outcome = frame.outcomes.pop().ok_or("missing kernel leaf outcome")?;
            frame.cursor = cursor;
            self.kernel_results()?.push(outcome);
            // Carry the native executor's updated facts and progress back to
            // the invocation. Only the outer instruction advances this cursor.
            let Some(Frame::Resolve { frame: root }) = self.rules_mut().frames.first_mut() else {
                return Err("missing kernel invocation".into());
            };
            *root = frame;
            self.kernel_leaf = None;
            self.return_kernel(resume)?;
        } else if !self
            .state
            .rules
            .frame
            .holds(&self.state.rules.frames, root.stack_id)
        {
            // Departure can cancel the invocation and its remaining instructions.
            self.kernel_leaf = None;
        }
        Ok(())
    }
}
