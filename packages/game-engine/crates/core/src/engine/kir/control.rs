//! A suspended node names its location in the admitted instruction. Sequences
//! and iterations save their remaining work on the same stack as rule frames.
//! No snapshot supplies an executable node.
//!
//! A creature-type choice binds a name from CR 205.3m for its child to read
//! (CR 608.2d). A singleton optional effect is skipped when its chooser leaves
//! (CR 800.4a). When each player or each opponent may draw, collect choices
//! in APNAP order and show earlier answers to the next chooser. Then draw in
//! APNAP order, keeping card identities private. A later instruction can read
//! the players who actually drew (CR 101.4, 121.2c, 608.2e).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use mtg_contracts::kir::{Cmp, Cond, CreatureType, Domain, Payment, Resolved};
mod evaluated;
use evaluated::{StepProgress, StepResults};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Binding {
    Player(PlayerId),
    Objects(Vec<ObjectId>),
    Revealed(Vec<ObjectId>),
    CreatureType(CreatureType),
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "stage", rename_all = "snake_case", deny_unknown_fields)]
enum Stage {
    #[default]
    Run,
    Leaf,
    Complete {
        outcomes: Vec<InstructionOutcome>,
        results: StepResults,
        first_batch: u64,
    },
    Step(StepProgress),
    MyriadDefender {
        chooser: PlayerId,
    },
    #[serde(untagged)]
    Ask(AskKind),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "stage", rename_all = "snake_case", deny_unknown_fields)]
enum AskKind {
    Optional {
        chooser: PlayerId,
    },
    /// CR 101.4, 121.2c: finish every optional draw choice before
    /// performing any player's draws in APNAP order.
    OptionalDraws {
        remaining: Vec<PlayerId>,
        choices: Vec<(PlayerId, bool)>,
    },
    CreatureType {
        chooser: PlayerId,
    },
    ObjectPayment {
        payer: PlayerId,
    },
    #[serde(rename = "life")]
    PayLife {
        payer: PlayerId,
        amount: Quantity,
    },
    #[serde(rename = "choose")]
    Pick {
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
    path: Vec<u32>,
    iterations: Vec<u32>,
    #[serde(deserialize_with = "read_bindings")]
    bindings: BTreeMap<u32, Binding>,
    stage: Stage,
}

// A tagged Frame buffers its payload before passing it to Task. Its map
// keys arrive as strings, so decode canonical numeric keys explicitly.
fn read_bindings<'de, D>(decoder: D) -> Result<BTreeMap<u32, Binding>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    BTreeMap::<String, Binding>::deserialize(decoder)?
        .into_iter()
        .map(|(key, value)| {
            let index = key.parse::<u32>().map_err(serde::de::Error::custom)?;
            if key != index.to_string() {
                return Err(serde::de::Error::custom(
                    "noncanonical kernel binding index",
                ));
            }
            Ok((index, value))
        })
        .collect()
}

impl Task {
    pub(in crate::engine) fn publicly_reveals(&self, object: ObjectId) -> bool {
        self.bindings
            .values()
            .any(|binding| matches!(binding, Binding::Revealed(cards) if cards.contains(&object)))
    }
    pub(in crate::engine) fn sacrifice_choices(&self) -> &[(PlayerId, ObjectId)] {
        match &self.stage {
            Stage::Step(StepProgress::Collect {
                chosen,
                private: false,
                ..
            }) => chosen,
            _ => &[],
        }
    }

    pub(in crate::engine) fn random_stream(&self) -> Option<u64> {
        match &self.stage {
            Stage::Step(StepProgress::Sample {
                sampling: Some(progress),
                ..
            }) => Some(progress.stream),
            _ => None,
        }
    }

    fn child(&self, index: u32) -> Self {
        let mut path = self.path.clone();
        path.push(index);
        Self {
            path,
            iterations: self.iterations.clone(),
            bindings: self.bindings.clone(),
            resume: self.resume,
            stage: Stage::Run,
        }
    }

    pub(in crate::engine) fn is_leaf(&self) -> bool {
        self.stage == Stage::Leaf
    }

    pub(in crate::engine) fn candidate(&self) -> Option<ObjectId> {
        if let Stage::Ask(AskKind::Pick { remaining, .. }) = &self.stage {
            remaining.last().copied()
        } else {
            None
        }
    }

    fn chooser(&self) -> Option<PlayerId> {
        match self.stage {
            Stage::Step(StepProgress::Collect { ref remaining, .. }) => remaining.first().copied(),
            Stage::Step(StepProgress::Moves {
                ref remaining,
                controller,
                ..
            }) => remaining.first().map(|(owner, _)| *owner).or(controller),
            Stage::Run | Stage::Leaf | Stage::Complete { .. } | Stage::Step(_) => None,
            Stage::Ask(AskKind::Optional { chooser })
            | Stage::MyriadDefender { chooser }
            | Stage::Ask(AskKind::CreatureType { chooser })
            | Stage::Ask(AskKind::Pick { chooser, .. }) => Some(chooser),
            Stage::Ask(AskKind::OptionalDraws { ref remaining, .. }) => remaining.first().copied(),
            Stage::Ask(AskKind::PayLife { payer, .. })
            | Stage::Ask(AskKind::ObjectPayment { payer }) => Some(payer),
        }
    }
}

impl RulesTxn {
    pub(super) fn begin_kernel_control(&mut self, frame: Resolving) -> Result<(), String> {
        let first_batch = self.state.rules.next_batch_id;
        let resume = self.save_frame(Frame::Resolve { frame });
        let root = Task {
            resume,
            path: Vec::new(),
            iterations: Vec::new(),
            bindings: BTreeMap::new(),
            stage: Stage::Complete {
                outcomes: Vec::new(),
                results: StepResults::default(),
                first_batch,
            },
        };
        let mut task = Task {
            stage: Stage::Run,
            ..root.clone()
        };
        task.resume = self.save_frame(Frame::Kernel {
            task: Box::new(root),
        });
        if let (Node::Do(verb), block) = self.node_at(&task)?
            && verb.is_evaluated()
        {
            return self.evaluate_step(task, &verb.clone(), block);
        }
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
        for &index in &task.path {
            node = match node {
                Node::Seq(nodes) => nodes.get(index as usize),
                Node::If {
                    then, otherwise, ..
                } => match index {
                    0 => Some(then.as_ref()),
                    1 => otherwise.as_deref(),
                    _ => None,
                },
                Node::ForEach { body, .. }
                | Node::Choose { body, .. }
                | Node::ChooseCreatureType { body, .. }
                | Node::RevealTop { body, .. }
                | Node::Optional { node: body, .. }
                | Node::UnlessPays { node: body, .. }
                | Node::IfPays { node: body, .. }
                    if index == 0 =>
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
            SetExpr::Revealed {
                var,
                filter,
                except,
            } => self.revealed_objects(task, *var, filter, *except)?,
            SetExpr::Result {
                step,
                filter,
                destination,
            } => match self.result_cards(task, *step, filter, destination.as_ref())? {
                Resolved::Present(objects) => objects,
                Resolved::Absent(_) => vec![],
            },
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

    fn revealed_objects(
        &self,
        task: &Task,
        var: u32,
        filter: &Pred,
        except: Option<u32>,
    ) -> Result<Vec<ObjectId>, String> {
        let objects = |var| match task.bindings.get(&var) {
            Some(Binding::Revealed(objects)) => Ok(objects.as_slice()),
            _ => Err("missing revealed library binding"),
        };
        let excluded = except.map(objects).transpose()?.unwrap_or_default();
        let you = self.entry(self.kernel_root()?.stack_id)?.controller;
        let mut selected = vec![];
        for &id in objects(var)? {
            if excluded.contains(&id)
                || !self
                    .current(id)
                    .is_ok_and(|obj| matches!(obj.zone, Zone::Library { .. }))
            {
                continue;
            }
            if filter.is_empty()
                || self.damage_fact_matches(filter, you, None, &self.object_before(id)?)
            {
                selected.push(id);
            }
        }
        Ok(selected)
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
            // CR 400.7, 608.2h, 712.8a: a card in a graveyard has its
            // physical front face. Read the bound incarnation even after
            // the move rather than the successor on the battlefield/exile.
            Cond::Satisfies {
                object: Ref::Target { slot },
                filter,
            } if filter.0.as_slice()
                == [mtg_contracts::kir::Atom::Type {
                    kind: CardKind::Creature,
                }] =>
            {
                let frame = self.kernel_root()?;
                let entry = self.entry(frame.stack_id)?;
                frame.legal_targets.get(*slot as usize) == Some(&true)
                    && self
                        .bound_object_target(entry, *slot as usize)
                        .is_some_and(|id| {
                            self.copiable_definition(id)
                                .is_ok_and(|d| d.characteristics.has_type(CardKind::Creature))
                        })
            }
            Cond::Satisfies { .. }
            | Cond::CastBy { .. }
            | Cond::TurnHistory { .. }
            | Cond::ControlsOwnCommander { .. }
            | Cond::OpponentControlsMore { .. }
            | Cond::EventOpponentControlsMore { .. }
            | Cond::ControllerCreatureTotalPower { .. }
            | Cond::Measured { .. }
            | Cond::ControlledSinceLastUpkeep {} => {
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
        if let Stage::Complete { mut outcomes, .. } = task.stage {
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
        let Stage::Complete { outcomes, .. } = &mut task.stage else {
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
        if matches!(task.stage, Stage::Step(StepProgress::Collect { .. })) {
            return self.collect_request(task);
        }
        if matches!(task.stage, Stage::Step(StepProgress::Moves { .. })) {
            return self.move_request(task);
        }
        if matches!(task.stage, Stage::MyriadDefender { .. }) {
            let candidates = self.myriad_candidates(task, player)?;
            return Ok((candidates.len() > 1)
                .then(|| self.pick(player, Reason::MyriadDefender, candidates, 1, 1)));
        }
        if matches!(task.stage, Stage::Ask(AskKind::ObjectPayment { .. })) {
            let candidates = self.object_payment_candidates(player, self.node_at(task)?.0)?;
            return Ok(Some(
                self.pick(
                    player,
                    Reason::OptionalEffect,
                    std::iter::once(Candidate::Flag {
                        flag: Flag::Decline,
                    })
                    .chain(
                        candidates
                            .into_iter()
                            .map(|object| Candidate::Object { object }),
                    )
                    .collect(),
                    1,
                    1,
                ),
            ));
        }
        if matches!(task.stage, Stage::Ask(AskKind::CreatureType { .. })) {
            return Ok(Some(
                self.pick(
                    player,
                    Reason::ChooseCreatureType,
                    CreatureType::all()
                        .map(|kind| Candidate::CreatureType { kind })
                        .collect(),
                    1,
                    1,
                ),
            ));
        }
        if let Stage::Ask(AskKind::OptionalDraws { choices, .. }) = &task.stage {
            return Ok(Some(self.pick_flags(
                player,
                Reason::OptionalResolution {
                    previous: choices.clone(),
                },
                &[Flag::Accept, Flag::Decline],
            )));
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
            (Stage::Ask(AskKind::Optional { .. }), Node::Optional { node, .. }) => {
                self.performable(task, node)
            }
            (Stage::Ask(AskKind::ObjectPayment { payer }), node)
                if Self::is_sacrifice_payment(node) =>
            {
                Ok(!self.object_payment_candidates(*payer, node)?.is_empty())
            }
            _ => Ok(true),
        }
    }

    fn is_sacrifice_payment(node: &Node) -> bool {
        matches!(
            node,
            Node::IfPays {
                payment: Payment::Sacrifice { .. },
                ..
            } | Node::UnlessPays {
                payment: Payment::Sacrifice { .. },
                ..
            }
        )
    }

    fn object_payment_candidates(
        &self,
        player: PlayerId,
        node: &Node,
    ) -> Result<Vec<ObjectId>, String> {
        if Self::is_sacrifice_payment(node) {
            return self.controlled_creatures(player);
        }
        let mut eligible = Vec::new();
        for card in self.action_hand(player) {
            if self
                .characteristics(card)?
                .has_type(mtg_contracts::CardKind::Artifact)
            {
                eligible.push(card);
            }
        }
        Ok(eligible)
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
        self.validate_step_address(task)?;
        match (&task.stage, node) {
            (Stage::Step(progress), Node::Do(verb @ Verb::Draw { .. }))
                if depth == self.state.rules.frames.len()
                    || matches!(self.state.rules.frame, Frame::CommanderReturn {
                        continuation: CommanderContinuation::ResolutionDraw { resume }, ..
                    } if resume.0 == depth) =>
            {
                self.validate_sequence(task, progress, verb)
            }
            (
                Stage::Step(progress @ StepProgress::Collect { .. }),
                Node::Do(Verb::Sacrifice { .. } | Verb::Discard { .. }),
            ) if depth == self.state.rules.frames.len() => self.validate_collect(task, progress),
            (
                Stage::Step(progress @ StepProgress::Sample { .. }),
                Node::Do(Verb::Discard { random: true, .. }),
            ) if depth == self.state.rules.frames.len() => self.validate_sample(task, progress),
            (Stage::Step(progress @ StepProgress::Moves { .. }), Node::Do(Verb::Move { .. }))
                if depth == self.state.rules.frames.len() =>
            {
                self.validate_moves(task, progress)
            }
            // A specialist transaction holds this saved task as its parent.
            (
                Stage::Step(
                    StepProgress::Damage { .. }
                    | StepProgress::Counters { .. }
                    | StepProgress::CommitMoves { .. },
                ),
                Node::Do(_),
            ) if self.specialist_resumes(depth) => Ok(()),

            (Stage::Complete { outcomes, .. }, _)
                if task.resume == FrameLink(0)
                    && task.path.is_empty()
                    && task.iterations.is_empty()
                    && task.bindings.is_empty()
                    && depth == 1
                    && depth < self.state.rules.frames.len() =>
            {
                self.validate_step_results(task)?;
                for outcome in outcomes {
                    self.validate_library_reorder_outcome(self.kernel_root()?, None, outcome)?;
                    self.validate_counter_outcome(root, outcome)?;
                    self.validate_myriad_outcome(root, outcome)?;
                }
                Ok(())
            }
            (Stage::Leaf, Node::Do(verb)) if !verb.is_evaluated() => Ok(()),
            (Stage::Run, _) => Ok(()),
            (Stage::MyriadDefender { chooser }, Node::Do(Verb::CreateMyriadToken { defender })) => {
                self.myriad_defender(task, defender)?;
                if self.entry(root.stack_id)?.controller != *chooser {
                    return Err("invalid myriad chooser".into());
                }
                Ok(())
            }
            // The saved chooser is the one the admitted node names.
            (Stage::Ask(AskKind::Optional { chooser }), Node::Optional { chooser: named, .. }) => {
                if self.control_players(task, named)? != [*chooser] {
                    return Err("kernel chooser differs from its node".into());
                }
                Ok(())
            }
            (
                Stage::Ask(AskKind::CreatureType { chooser }),
                Node::ChooseCreatureType { chooser: named, .. },
            ) if self.control_players(task, named)? == [*chooser] => Ok(()),
            (
                Stage::Ask(AskKind::OptionalDraws { remaining, choices }),
                Node::Optional {
                    chooser: named @ (PlayerExpr::EachPlayer | PlayerExpr::EachOpponent),
                    node,
                },
            ) if matches!(node.verb(), Some(Verb::Draw { player, .. }) if player == named) => {
                let controller = self.entry(root.stack_id)?.controller;
                let mut ordered = choices
                    .iter()
                    .map(|(player, _)| *player)
                    .chain(remaining.iter().copied());
                let mut next = ordered.next();
                for player in self
                    .apnap_seats()
                    .filter(|&player| *named == PlayerExpr::EachPlayer || player != controller)
                {
                    if next == Some(player) {
                        next = ordered.next();
                    }
                }
                if remaining.is_empty() || next.is_some() {
                    return Err("invalid optional draw choices".into());
                }
                Ok(())
            }
            (
                Stage::Ask(AskKind::ObjectPayment { payer }),
                Node::IfPays {
                    payer: named,
                    payment,
                    ..
                }
                | Node::UnlessPays {
                    payer: named,
                    payment,
                    ..
                },
            ) if matches!(
                payment,
                Payment::Sacrifice { .. } | Payment::DiscardArtifact {}
            ) =>
            {
                if self.control_players(task, named)? != [*payer] {
                    return Err("kernel object payment differs from its node".into());
                }
                Ok(())
            }
            // CR 118.12: the saved payer and amount are the admitted terms.
            (
                Stage::Ask(AskKind::PayLife { payer, amount }),
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
                Stage::Ask(AskKind::Pick {
                    chooser,
                    remaining,
                    selected,
                    minimum,
                    maximum,
                    ..
                }),
                Node::Choose {
                    chooser: named,
                    from,
                    ..
                },
            ) if minimum <= maximum
                && remaining
                    .iter()
                    .chain(selected)
                    .collect::<BTreeSet<_>>()
                    .len()
                    == remaining.len() + selected.len()
                && !remaining.is_empty()
                && selected.len() < *maximum
                && selected.len() + remaining.len() > *minimum =>
            {
                if self.control_players(task, named)? != [*chooser] {
                    return Err("kernel choice differs from its chooser".into());
                }
                let permitted = self.control_objects(task, from)?;
                if remaining
                    .iter()
                    .chain(selected)
                    .any(|id| self.current(*id).is_ok() && !permitted.contains(id))
                {
                    return Err("kernel choice includes an unrelated object".into());
                }
                Ok(())
            }
            _ => Err("invalid kernel suspension".into()),
        }
    }

    pub(in crate::engine) fn step_kernel_task(&mut self, mut task: Task) -> Result<(), String> {
        let (node, block) = self.node_at(&task)?;
        let node = node.clone();
        match &task.stage {
            Stage::Step(StepProgress::Sequence { .. }) => return self.step_sequence(task),
            Stage::Step(StepProgress::Collect { .. }) => return self.step_collect(task),
            Stage::Step(StepProgress::Sample { .. }) => return self.step_sample(task),
            Stage::Step(StepProgress::Moves { .. }) => return self.step_moves(task),
            Stage::Step(_) => return Err("a specialist transaction resumes this step".into()),
            _ => {}
        }
        if let Stage::MyriadDefender { chooser } = task.stage {
            let candidates = self.myriad_candidates(&task, chooser)?;
            let input = self.rules_mut().pending.take();
            let selected = match candidates.as_slice() {
                [] => return self.return_kernel(task.resume),
                [only] => only.clone(),
                _ => input
                    .ok_or("missing myriad defender decision")?
                    .one()?
                    .clone(),
            };
            if !candidates.contains(&selected) {
                return Err("ineligible myriad defender".into());
            }
            let Some(Candidate::Player { player }) = candidates.first() else {
                return Err("missing myriad opponent".into());
            };
            let planeswalker = match selected {
                Candidate::Player { .. } => None,
                Candidate::Object { object } => Some(object),
                _ => return Err("invalid myriad defender".into()),
            };
            return self.accept_myriad(
                task,
                MyriadDefender {
                    player: *player,
                    planeswalker,
                },
            );
        }
        if let Stage::Ask(AskKind::ObjectPayment { payer }) = task.stage {
            let input = self.rules_mut().pending.take();
            let paid = if self.is_live(payer) && self.still_offered(&task)? {
                match input.ok_or("missing object payment decision")?.one()? {
                    Candidate::Flag {
                        flag: Flag::Decline,
                    } => false,
                    Candidate::Object { object } => {
                        if !self
                            .object_payment_candidates(payer, &node)?
                            .contains(object)
                        {
                            return Err("ineligible kernel object payment".into());
                        }
                        let proposed = if Self::is_sacrifice_payment(&node) {
                            ProposedEvent::Sacrifice {
                                object: *object,
                                player: payer,
                            }
                        } else {
                            ProposedEvent::DiscardCard {
                                object: *object,
                                player: payer,
                            }
                        };
                        matches!(
                            self.commit(
                                proposed,
                                self.instruction_cause(self.kernel_root()?, block)?
                            )?,
                            EventOutcome::Sacrificed { .. } | EventOutcome::CardDiscarded { .. }
                        )
                    }
                    _ => return Err("invalid object payment decision".into()),
                }
            } else {
                false
            };
            return self.paid(task, &node, paid);
        }
        if let Stage::Ask(AskKind::CreatureType { chooser }) = task.stage {
            let input = self.rules_mut().pending.take();
            if !self.is_live(chooser) {
                return self.return_kernel(task.resume);
            }
            let input = input.ok_or("missing creature type decision")?;
            let Candidate::CreatureType { kind } = input.one()? else {
                return Err("expected a creature type".into());
            };
            let Node::ChooseCreatureType { var, .. } = node else {
                return Err("missing creature type choice node".into());
            };
            let mut child = task.child(0);
            child.bindings.insert(var, Binding::CreatureType(*kind));
            self.install_kernel(child);
            return Ok(());
        }
        if let Some(player) = task.chooser() {
            let offered = self.is_live(player) && self.still_offered(&task)?;
            let perform = if offered {
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
                Stage::Ask(AskKind::Optional { .. }) if perform => {
                    self.install_kernel(task.child(0))
                }
                Stage::Ask(AskKind::Optional { .. }) => return self.return_kernel(task.resume),
                Stage::Ask(AskKind::OptionalDraws {
                    mut remaining,
                    mut choices,
                }) => {
                    remaining.remove(0);
                    if offered {
                        choices.push((player, perform));
                    }
                    if remaining.is_empty() {
                        let accepted = choices
                            .into_iter()
                            .filter_map(|(player, chose)| chose.then_some(player))
                            .collect();
                        return self.perform_optional_draws(task.child(0), accepted);
                    }
                    task.stage = Stage::Ask(AskKind::OptionalDraws { remaining, choices });
                    self.install_kernel(task);
                }
                // CR 118.12: "unless" performs the node when the payer
                // declines; "if they do" performs it once they have paid.
                Stage::Ask(AskKind::PayLife { amount, .. }) if perform => {
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
                Stage::Ask(AskKind::PayLife { .. }) => return self.paid(task, &node, false),
                Stage::Ask(AskKind::Pick {
                    mut remaining,
                    mut selected,
                    minimum,
                    maximum,
                    ..
                }) => {
                    let object = remaining.pop().ok_or("missing kernel candidate")?;
                    if perform {
                        selected.push(object);
                    }
                    let Node::Choose { var, .. } = node else {
                        return Err("missing choose node".into());
                    };
                    self.choose_next(task, var, player, remaining, selected, minimum, maximum)?;
                }
                Stage::Run
                | Stage::Leaf
                | Stage::Step(_)
                | Stage::Complete { .. }
                | Stage::Ask(AskKind::CreatureType { .. })
                | Stage::Ask(AskKind::ObjectPayment { .. })
                | Stage::MyriadDefender { .. } => {
                    return Err("unexpected kernel input".into());
                }
            }
            return Ok(());
        }
        match &node {
            Node::Seq(nodes) => {
                self.schedule(task.resume, (0..nodes.len() as u32).map(|i| task.child(i)))?
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
                if matches!(chooser, PlayerExpr::EachPlayer | PlayerExpr::EachOpponent) {
                    if !self.performable(&task, child)? {
                        return self.return_kernel(task.resume);
                    }
                    let remaining = self.control_players(&task, chooser)?;
                    if remaining.is_empty() {
                        return self.return_kernel(task.resume);
                    }
                    task.stage = Stage::Ask(AskKind::OptionalDraws {
                        remaining,
                        choices: Vec::new(),
                    });
                    self.install_kernel(task);
                    return Ok(());
                }
                let Some(chooser) = self.chooser(&task, chooser)? else {
                    return self.return_kernel(task.resume);
                };
                // CR 608.2d, 701.26a: no choice is offered to perform an
                // action that cannot be performed.
                if !self.performable(&task, child)? {
                    return self.return_kernel(task.resume);
                }
                task.stage = Stage::Ask(AskKind::Optional { chooser });
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
                    members.into_iter().enumerate().map(|(ordinal, member)| {
                        let mut child = task.child(0);
                        child.iterations.push(ordinal as u32);
                        child.bindings.insert(*var, member);
                        child
                    }),
                )?;
            }
            Node::ChooseCreatureType { chooser, .. } => {
                let Some(chooser) = self.chooser(&task, chooser)? else {
                    return self.return_kernel(task.resume);
                };
                task.stage = Stage::Ask(AskKind::CreatureType { chooser });
                self.install_kernel(task);
            }
            Node::RevealTop {
                var, player, count, ..
            } => {
                // CR 701.20c: revealing leaves each card in its place.
                // A token that was put in the library is not a card (111.6).
                let Resolved::Present(player) = self.measured_player(&task, player)? else {
                    return self.return_kernel(task.resume);
                };
                if !self.is_live(player) {
                    return self.return_kernel(task.resume);
                }
                let objects = self.state.rules.libraries[&player]
                    .iter()
                    .filter(|card| !self.state.rules.tokens.contains_key(card))
                    .take(*count as usize)
                    .map(|card| self.object_id(*card).ok_or("missing revealed library card"))
                    .collect::<Result<Vec<_>, _>>()?;
                for &object in &objects {
                    self.remember_revealed_library_top(object);
                }
                let mut child = task.child(0);
                child.bindings.insert(*var, Binding::Revealed(objects));
                self.install_kernel(child);
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
                if !matches!(from, SetExpr::Revealed { .. }) {
                    let mut candidates = objects
                        .into_iter()
                        .map(|object| Ok((self.handle_for(player, object)?.0, object)))
                        .collect::<Result<Vec<_>, String>>()?;
                    candidates.sort_by(|left, right| left.0.cmp(&right.0));
                    objects = candidates.into_iter().map(|(_, object)| object).collect();
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
                    // Keep a private discard offer even with no matching card.
                    // CR 400.2: only the payer may see the eligible hand objects.
                    Payment::Sacrifice { .. } | Payment::DiscardArtifact {} => {
                        task.stage = Stage::Ask(AskKind::ObjectPayment { payer: player });
                        self.install_kernel(task);
                    }
                    Payment::Life { amount } => {
                        let amount = self.control_amount(&task, amount)?;
                        if amount.is_negative() {
                            return Err("negative kernel life payment".into());
                        }
                        // CR 119.4: a player can't pay more life than they have.
                        if self.state.rules.players[player.0 as usize].can_pay_life(&amount) {
                            task.stage = Stage::Ask(AskKind::PayLife {
                                payer: player,
                                amount,
                            });
                            self.install_kernel(task);
                        } else {
                            self.paid(task, &node, false)?;
                        }
                    }
                    Payment::Mana { mana: cost } => {
                        // CR 605.3a: the payer may activate mana abilities
                        // here. With no usable source, only the pool can fund it.
                        if self.can_pay_for(player, cost, payments::ManaUse::Resolution)?
                            || self.state.rules.objects.values().any(|object| {
                                self.validate_mana_activation(player, object.id).is_ok()
                            })
                        {
                            self.begin_kernel_payment(task, player, cost.clone())?;
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
            Node::Do(verb) if verb.is_evaluated() => {
                self.evaluate_step(task, verb, block)?;
            }
            // CR 702.116a: choose a defender for each accepted copy before
            // creating all copies together (CR 608.2f).
            Node::Do(Verb::CreateMyriadToken { .. }) => {
                let chooser = self.entry(self.kernel_root()?.stack_id)?.controller;
                match self.myriad_candidates(&task, chooser)?.as_slice() {
                    [] => self.return_kernel(task.resume)?,
                    [Candidate::Player { player }] => self.accept_myriad(
                        task,
                        MyriadDefender {
                            player: *player,
                            planeswalker: None,
                        },
                    )?,
                    _ => {
                        task.stage = Stage::MyriadDefender { chooser };
                        self.install_kernel(task);
                    }
                }
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
            Node::Delayed { .. }
            | Node::ChooseObject { .. }
            | Node::TapTargets { .. }
            | Node::SacrificeForTappedReturn { .. }
            | Node::RemoveTargets { .. } => {
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
    /// * Transform: the source can transform now (CR 701.27c-f).
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
            // A local dependent move can only become possible once the
            // earlier step of this optional sequence has been performed.
            Node::Do(Verb::Move {
                what: SetExpr::LiveResult { .. },
                ..
            }) => false,
            Node::Do(Verb::Tap { what }) => present(what, &|o| !o.tapped)?,
            Node::Do(Verb::Untap { what }) => present(what, &|o| o.tapped)?,
            Node::Do(Verb::CreateMyriadToken { defender }) => {
                self.myriad_defender(task, defender)?.is_some()
            }
            // CR 701.27c-f: the source can still turn over.
            Node::Do(Verb::Transform { what: Ref::Source }) => {
                self.source_transformable(self.entry(self.kernel_root()?.stack_id)?)?
            }
            Node::Do(verb) if verb.is_evaluated() => self.evaluated_performable(task, verb)?,
            // Composition admits optional bodies of the verbs above only
            // (`compose::control::decided`); nested control decides its own
            // choices when it runs.
            _ => true,
        })
    }

    fn accept_myriad(&mut self, task: Task, defender: MyriadDefender) -> Result<(), String> {
        self.kernel_results()?
            .push(InstructionOutcome::MyriadAccepted { defender });
        self.return_kernel(task.resume)
    }

    fn myriad_candidates(
        &self,
        task: &Task,
        chooser: PlayerId,
    ) -> Result<Vec<Candidate<Rules>>, String> {
        let Node::Do(Verb::CreateMyriadToken { defender }) = self.node_at(task)?.0 else {
            return Err("myriad choice requires a token node".into());
        };
        let Some(player) = self.myriad_defender(task, defender)? else {
            return Ok(vec![]);
        };
        let mut walkers = self
            .state
            .rules
            .objects
            .values()
            .filter(|o| o.controller == player && self.is_battlefield_planeswalker(o.id))
            .map(|o| Ok((self.object_handle_id(chooser, o.id)?.clone(), o.id)))
            .collect::<Result<Vec<_>, String>>()?;
        walkers.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(std::iter::once(Candidate::Player { player })
            .chain(
                walkers
                    .into_iter()
                    .map(|(_, object)| Candidate::Object { object }),
            )
            .collect())
    }

    /// CR 702.116a: the bound player, if they are a live opponent of the
    /// ability's controller other than the defending player its source
    /// attacked. Myriad offers no choice for anyone else (CR 608.2d).
    fn myriad_defender(
        &self,
        task: &Task,
        defender: &PlayerExpr,
    ) -> Result<Option<PlayerId>, String> {
        let [player] = self.control_players(task, defender)?[..] else {
            return Err("myriad requires one bound player".into());
        };
        let entry = self.entry(self.kernel_root()?.stack_id)?;
        let StackKind::Triggered {
            witness:
                TriggerEventWitness::CreatureAttacked {
                    defender_player_id, ..
                },
            ..
        } = &entry.kind
        else {
            return Err("myriad requires an attack trigger".into());
        };
        Ok((self.is_live(player)
            && self.is_live(entry.controller)
            && player != entry.controller
            && player != *defender_player_id)
            .then_some(player))
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
            Node::UnlessPays { payer, payment, .. } | Node::IfPays { payer, payment, .. } => {
                let players = self.control_players(task, payer)?;
                let [player] = players.as_slice() else {
                    return Err("invalid kernel payer".into());
                };
                let cost = match payment {
                    Payment::Generic { amount } => ManaCost {
                        generic: self.control_amount(task, amount)?,
                        colored: vec![],
                    },
                    Payment::Mana { mana: cost } => cost.clone(),
                    Payment::Life { .. }
                    | Payment::Sacrifice { .. }
                    | Payment::DiscardArtifact {} => {
                        return Err("invalid kernel mana payment".into());
                    }
                };
                Ok((*player, block, cost))
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
            let binding = match self.node_at(&task)?.0 {
                Node::Choose {
                    from: SetExpr::Revealed { .. },
                    ..
                } => Binding::Revealed(selected),
                _ => Binding::Objects(selected),
            };
            child.bindings.insert(var, binding);
            self.install_kernel(child);
        } else {
            task.stage = Stage::Ask(AskKind::Pick {
                chooser,
                remaining,
                selected,
                minimum,
                maximum,
            });
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
