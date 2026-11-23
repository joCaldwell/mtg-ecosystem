use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Budget {
    remaining: u64,
}
impl Budget {
    pub fn new(units: u64) -> Self {
        Self { remaining: units }
    }
    pub fn unlimited() -> Self {
        Self::new(u64::MAX)
    }
    pub(crate) fn consume(&mut self) -> bool {
        if self.remaining == 0 {
            false
        } else {
            self.remaining -= 1;
            true
        }
    }
}
pub enum QueryValue {
    Observation(Observation),
    Decision(Option<PlayerRequest>),
    Validated(ValidatedResponse),
    SearchContext(SearchContext),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueryRejection {
    UnavailableDuringSetup,
    InvalidViewer,
    InvalidResponse,
    UnsupportedBoundary,
    StaleOrForeignWork,
    InvalidRecoveryWork,
    Invariant,
}
pub enum QueryResult {
    Complete(QueryValue),
    Rejected(QueryRejection),
    Incomplete { work: Work, required_minimum: u64 },
}
testkit_fields! {
    pub struct Work {
        executor: Arc<()>,
        state_token: Arc<()>,
        data: WorkData,
        cache: Box<WorkCache>,
    }
}

impl Work {
    pub(crate) fn belongs_to_viewer(&self, viewer: PlayerId) -> bool {
        let (Query::Observe { viewer: owner }
        | Query::Decision { viewer: owner }
        | Query::Check { viewer: owner, .. }
        | Query::Search { viewer: owner, .. }) = &self.data.query;
        *owner == viewer
    }
}

// Pure derived accelerators live only in opaque, source/state-scoped Work.
// They never enter SavedWork or semantic State. Initialization remains lazy:
// Budget0 does not enumerate anything. The first existing unit computes each
// domain exactly as before; later units reuse it without changing row counts.
#[derive(Default)]
pub struct WorkCache {
    pub projection: Option<visibility::ProjectionIndex>,
    pub last_object: Option<CardId>,
    pub effects: Option<Vec<EffectKey>>,
    triggers: Option<Vec<TriggerId>>,
    modes: Option<Option<Vec<u32>>>,
    counter_placement: Option<Vec<CounterPlacementEffectKey>>,
    damage: Option<Vec<DamageEffectCandidate>>,

    entry_candidates: Option<Vec<ObjectId>>,
}
impl WorkCache {
    fn projection(&mut self, game: &RulesTxn) -> &visibility::ProjectionIndex {
        self.projection
            .get_or_insert_with(|| game.projection_index())
    }
    fn next_object<'a>(&mut self, game: &'a RulesTxn) -> Option<&'a Object> {
        use std::ops::Bound::{Excluded, Unbounded};
        let next = match self.last_object {
            None => game.state.rules.objects.first_key_value(),
            Some(last) => game
                .state
                .rules
                .objects
                .range((Excluded(last), Unbounded))
                .next(),
        };
        if let Some((key, _)) = next {
            self.last_object = Some(key);
        }
        next.map(|(_, object)| object)
    }
    fn effects(&mut self, game: &RulesTxn) -> &[EffectKey] {
        self.effects.get_or_insert_with(|| game.live_effect_keys())
    }
    fn triggers(&mut self, game: &RulesTxn) -> &[TriggerId] {
        self.triggers
            .get_or_insert_with(|| game.trigger_order_ids())
    }
    fn modes(&mut self, game: &RulesTxn) -> Result<Option<&[u32]>, QueryRejection> {
        if self.modes.is_none() {
            self.modes = Some(
                match game
                    .modes_request()
                    .map_err(|_| QueryRejection::Invariant)?
                {
                    Some(Request::Modes { candidates, .. }) => Some(candidates),
                    _ => None,
                },
            );
        }
        Ok(self.modes.as_ref().and_then(|ids| ids.as_deref()))
    }

    fn entry_candidates(
        &mut self,
        game: &RulesTxn,
        viewer: PlayerId,
    ) -> Result<&[ObjectId], QueryRejection> {
        if self.entry_candidates.is_none() {
            self.entry_candidates = Some(
                game.entry_work_candidates(viewer)
                    .map_err(|_| QueryRejection::Invariant)?,
            );
        }
        Ok(self.entry_candidates.as_deref().unwrap_or_default())
    }
    fn counter_placement(&mut self, game: &RulesTxn) -> &[CounterPlacementEffectKey] {
        self.counter_placement
            .get_or_insert_with(|| game.counter_placement_candidates())
    }
    fn damage(&mut self, game: &RulesTxn) -> Result<&[DamageEffectCandidate], QueryRejection> {
        if self.damage.is_none() {
            self.damage = Some(
                game.damage_effect_group()
                    .map_err(|_| QueryRejection::Invariant)?
                    .map_or(vec![], |(_, _, pairs)| pairs),
            );
        }
        Ok(self.damage.as_deref().unwrap_or_default())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "query", rename_all = "snake_case", deny_unknown_fields)]
pub enum Query {
    Observe {
        viewer: PlayerId,
    },
    Decision {
        viewer: PlayerId,
    },
    Check {
        viewer: PlayerId,
        token: DecisionToken,
        response: PlayerResponse,
    },
    Search {
        viewer: PlayerId,
        sampling_seed: String,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservationWork {
    pub cursor: u32,
    pub view: PermittedView,
    pub decision: Option<PlayerRequest>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "task", rename_all = "snake_case", deny_unknown_fields)]
pub enum Task {
    // Sidecar v37 includes copied spells in public stack observations.
    // Projection/request headers remain explicit work, never hidden work
    // performed even when the caller grants no budget.
    Initialize,
    Observe {
        builder: ObservationWork,
    },
    Decision {
        cursor: u32,
        request: Option<PlayerRequest>,
    },
    Check {
        header_checked: bool,
        converted: Vec<ObjectId>,
        converted_entities: Vec<TargetEntity>,
        converted_triggers: Vec<TriggerId>,
        value: Option<Response>,
    },
    Search {
        builder: ObservationWork,
        observation: Option<Box<Observation>>,
        history: PermittedHistory,
        decks: Vec<OpenDecklist>,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkData {
    pub query: Query,
    pub task: Task,
    pub units_done: u64,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SavedWork {
    pub work_version: u32,
    pub build_id: String,
    pub catalog_digest: String,
    pub state_sha256: String,
    pub data: WorkData,
}

impl RulesTxn {
    /// When the entire observation fits and the decision has no candidate
    /// enumeration, the last published history row is the completed builder.
    /// Partial budgets still produce their original resumable work prefix.
    pub(in crate::engine) fn observe_committed(
        &self,
        viewer: PlayerId,
        budget: Budget,
    ) -> QueryResult {
        let work = match self.start_work(Query::Observe { viewer }) {
            Ok(work) => work,
            Err(error) => return QueryResult::Rejected(error),
        };
        if self.published.is_some()
            && let Some(row) = self.state.knowledge.visibility[viewer.0 as usize]
                .history
                .last()
            && budget.remaining
                >= 5 + self.state.rules.objects.len() as u64
                    + self.state.rules.stack.len() as u64
                    + row.effects.len() as u64
            && let Ok(
                decision @ (None
                | Some(
                    PlayerRequest::Priority { .. }
                    | PlayerRequest::Mana { .. }
                    | PlayerRequest::SpendMana { .. }
                    | PlayerRequest::OptionalEffect { .. }
                    | PlayerRequest::Attackers { .. }
                    | PlayerRequest::Blockers { .. },
                )),
            ) = self.request_header(viewer)
        {
            let observation = Observation {
                observation_version: crate::formats::OBSERVATION_VERSION,
                decision_context: self.permitted_decision_context(viewer, decision.as_ref()),
                view: row.to_public(),
                decision,
            };
            #[cfg(debug_assertions)]
            {
                let QueryResult::Complete(QueryValue::Observation(full)) =
                    self.resume_work(work, budget)
                else {
                    panic!("committed observation exceeded its builder budget or was rejected");
                };
                assert_eq!(
                    observation, full,
                    "committed observation differs from full builder"
                );
            }
            return QueryResult::Complete(QueryValue::Observation(observation));
        }
        self.resume_work(work, budget)
    }
    pub fn observe(&self, viewer: PlayerId, budget: Budget) -> QueryResult {
        self.begin_query(Query::Observe { viewer }, budget)
    }
    pub fn player_decision_for(&self, viewer: PlayerId, budget: Budget) -> QueryResult {
        self.begin_query(Query::Decision { viewer }, budget)
    }
    pub fn check_player_response(
        &self,
        viewer: PlayerId,
        token: DecisionToken,
        response: PlayerResponse,
        budget: Budget,
    ) -> QueryResult {
        self.begin_query(
            Query::Check {
                viewer,
                token,
                response,
            },
            budget,
        )
    }
    pub fn search_context(
        &self,
        viewer: PlayerId,
        sampling_seed: String,
        budget: Budget,
    ) -> QueryResult {
        self.begin_query(
            Query::Search {
                viewer,
                sampling_seed,
            },
            budget,
        )
    }
    pub(crate) fn start_work(&self, query: Query) -> Result<Work, QueryRejection> {
        let viewer = match &query {
            Query::Observe { viewer }
            | Query::Decision { viewer }
            | Query::Check { viewer, .. }
            | Query::Search { viewer, .. } => *viewer,
        };
        if self.setup_automatic() {
            return Err(QueryRejection::UnavailableDuringSetup);
        }
        if viewer.0 >= 4 {
            return Err(QueryRejection::InvalidViewer);
        }
        if !self.state.rules.pending_concessions.is_empty() {
            return Err(QueryRejection::UnsupportedBoundary);
        }
        if let Query::Search { sampling_seed, .. } = &query
            && (!matches!(self.state.rules.frame, Frame::Priority)
                || self.state.rules.pending.is_some()
                || !self.is_live(viewer)
                || sampling_seed.is_empty()
                || sampling_seed.len() > 128)
        {
            return Err(QueryRejection::UnsupportedBoundary);
        }
        Ok(Work {
            executor: self.executor.clone(),
            state_token: self.state_token.clone(),
            data: WorkData {
                query,
                task: Task::Initialize,
                units_done: 0,
            },
            cache: Box::default(),
        })
    }
    pub(crate) fn initial_query_task(&self, query: &Query) -> Result<Task, QueryRejection> {
        let viewer = match query {
            Query::Observe { viewer }
            | Query::Decision { viewer }
            | Query::Check { viewer, .. }
            | Query::Search { viewer, .. } => *viewer,
        };
        let observe = || -> Result<ObservationWork, QueryRejection> {
            Ok(ObservationWork {
                cursor: 0,
                view: self
                    .blank_view(viewer)
                    .map_err(|_| QueryRejection::Invariant)?,
                decision: self
                    .request_header(viewer)
                    .map_err(|_| QueryRejection::Invariant)?,
            })
        };
        let task = match query {
            Query::Observe { .. } => Task::Observe {
                builder: observe()?,
            },
            Query::Decision { .. } => Task::Decision {
                cursor: 0,
                request: self
                    .request_header(viewer)
                    .map_err(|_| QueryRejection::Invariant)?,
            },
            Query::Check { .. } => Task::Check {
                header_checked: false,
                converted: vec![],
                converted_entities: vec![],
                converted_triggers: vec![],
                value: None,
            },
            Query::Search { .. } => Task::Search {
                builder: observe()?,
                observation: None,
                history: PermittedHistory::default(),
                decks: vec![],
            },
        };
        Ok(task)
    }
    fn begin_query(&self, query: Query, budget: Budget) -> QueryResult {
        match self.start_work(query) {
            Ok(work) => self.resume_work(work, budget),
            Err(e) => QueryResult::Rejected(e),
        }
    }
    pub fn resume_work(&self, mut work: Work, mut budget: Budget) -> QueryResult {
        if !Arc::ptr_eq(&work.executor, &self.executor)
            || !Arc::ptr_eq(&work.state_token, &self.state_token)
        {
            return QueryResult::Rejected(QueryRejection::StaleOrForeignWork);
        }
        while budget.consume() {
            let Some(units) = work.data.units_done.checked_add(1) else {
                return QueryResult::Rejected(QueryRejection::Invariant);
            };
            work.data.units_done = units;
            match self.query_unit(&mut work.data, &mut work.cache) {
                Ok(Some(value)) => return QueryResult::Complete(value),
                Ok(None) => {}
                Err(e) => return QueryResult::Rejected(e),
            }
        }
        QueryResult::Incomplete {
            work,
            required_minimum: 1,
        }
    }
    fn observation_unit(
        &self,
        b: &mut ObservationWork,
        cache: &mut WorkCache,
    ) -> Result<bool, QueryRejection> {
        let cursor = b.cursor as usize;
        let viewer = b.view.viewer;
        if cursor < 4 {
            if let Some(PlayerRequest::EntityTargets { candidates, .. }) = &mut b.decision
                && self.targetable_player(PlayerId(cursor as u8))
            {
                candidates.push(PlayerTargetEntity::Player {
                    player: PlayerId(cursor as u8),
                });
            }
            b.view.players.push(
                self.visible_player_indexed(PlayerId(cursor as u8), cache.projection(self))
                    .map_err(|_| QueryRejection::Invariant)?,
            );
        } else if cursor < 4 + self.state.rules.objects.len() {
            if let Some(object) = cache.next_object(self)
                && let Some(visible) = self
                    .visible_object_indexed(viewer, object, cache.projection(self))
                    .map_err(|_| QueryRejection::Invariant)?
            {
                if let Some(PlayerRequest::BottomOpening { candidates, .. }) = &mut b.decision
                    && object.zone == (Zone::Hand { owner: viewer })
                {
                    candidates.push(visible.handle.clone());
                }
                if let Some(PlayerRequest::Targets { candidates, .. }) = &mut b.decision
                    && self
                        .top()
                        .is_ok_and(|entry| self.object_target_legal(entry, object.id))
                {
                    candidates.push(visible.handle.clone());
                }
                if let Some(PlayerRequest::EntityTargets { candidates, .. }) = &mut b.decision
                    && self.top().is_ok_and(|entry| {
                        self.entity_target_legal(TargetEntity::Object { object: object.id }, entry)
                    })
                {
                    candidates.push(PlayerTargetEntity::Object {
                        object: visible.handle.clone(),
                    });
                }
                if visible.zone == Zone::Exile {
                    b.view.players[visible.owner.0 as usize]
                        .exile
                        .push(visible.handle.clone());
                }
                b.view.objects.push(visible);
            }
        } else if cursor < 4 + self.state.rules.objects.len() + self.state.rules.stack.len() {
            b.view.stack.push(
                self.visible_stack(viewer, cursor - 4 - self.state.rules.objects.len())
                    .map_err(|_| QueryRejection::Invariant)?,
            );
        }
        let base = 4 + self.state.rules.objects.len() + self.state.rules.stack.len();
        let live_effect_count = cache.effects(self).len();
        let effect_count = live_effect_count + self.state.rules.delayed_chapters.len();
        if let Some(PlayerRequest::OrderTriggers { candidates, .. }) = &mut b.decision {
            let offset = base + effect_count;
            if cursor >= offset
                && let Some(id) = cache.triggers(self).get(cursor - offset)
            {
                candidates.push(
                    self.visible_pending_trigger(viewer, *id)
                        .map_err(|_| QueryRejection::Invariant)?,
                );
            }
        }
        b.cursor = b.cursor.checked_add(1).ok_or(QueryRejection::Invariant)?;
        if cursor >= base && cursor < base + effect_count {
            let index = cursor - base;
            let effect = if index < live_effect_count {
                self.visible_effect(viewer, &cache.effects(self)[index])
            } else {
                self.visible_delayed_chapter(
                    viewer,
                    &self.state.rules.delayed_chapters[index - live_effect_count],
                )
            };
            b.view
                .effects
                .push(effect.map_err(|_| QueryRejection::Invariant)?);
        }
        if let Some(
            PlayerRequest::EntryCopy { candidates, .. }
            | PlayerRequest::EntryAttachment { candidates, .. }
            | PlayerRequest::EntryReveal { candidates, .. },
        ) = &mut b.decision
            && cursor >= base + effect_count
            && let Some(&object) = cache
                .entry_candidates(self, viewer)?
                .get(cursor - base - effect_count)
        {
            candidates.push(
                self.handle_for(viewer, object)
                    .map_err(|_| QueryRejection::Invariant)?,
            );
        }

        if let Some(PlayerRequest::CounterPlacementEffect { candidates, .. }) = &mut b.decision
            && cursor >= base + effect_count
            && let Some(key) = cache
                .counter_placement(self)
                .get(cursor - base - effect_count)
        {
            candidates.push(
                self.visible_counter_candidate(viewer, key)
                    .map_err(|_| QueryRejection::Invariant)?,
            );
        }
        if let Some(PlayerRequest::DamageEffect { candidates, .. }) = &mut b.decision
            && cursor >= base + effect_count
            && let Some(pair) = cache.damage(self)?.get(cursor - base - effect_count)
        {
            candidates.push(
                self.visible_damage_effect_candidate(viewer, pair)
                    .map_err(|_| QueryRejection::Invariant)?,
            );
        }
        if let Some(PlayerRequest::Modes { candidates, .. }) = &mut b.decision
            && cursor >= base + effect_count
            && let Some(ids) = cache.modes(self)?
            && let Some(id) = ids.get(cursor - base - effect_count)
        {
            candidates.push(*id);
        }

        let extra = effect_count
            + if matches!(
                b.decision,
                Some(
                    PlayerRequest::EntryCopy { .. }
                        | PlayerRequest::EntryAttachment { .. }
                        | PlayerRequest::EntryReveal { .. }
                )
            ) {
                cache.entry_candidates(self, viewer)?.len()
            } else if matches!(b.decision, Some(PlayerRequest::Modes { .. })) {
                cache.modes(self)?.map_or(0, |ids| ids.len())
            } else if matches!(
                b.decision,
                Some(PlayerRequest::CounterPlacementEffect { .. })
            ) {
                cache.counter_placement(self).len()
            } else if matches!(b.decision, Some(PlayerRequest::DamageEffect { .. })) {
                cache.damage(self)?.len()
            } else if matches!(b.decision, Some(PlayerRequest::OrderTriggers { .. })) {
                cache.triggers(self).len()
            } else {
                0
            };
        if b.cursor as usize
            >= 4 + self.state.rules.objects.len() + self.state.rules.stack.len() + extra
        {
            Self::order_view_objects(&mut b.view);
            return Ok(true);
        }
        Ok(false)
    }
    fn query_unit(
        &self,
        data: &mut WorkData,
        cache: &mut WorkCache,
    ) -> Result<Option<QueryValue>, QueryRejection> {
        if matches!(data.task, Task::Initialize) {
            data.task = self.initial_query_task(&data.query)?;
            return Ok(None);
        }
        let invalid = |_: String| QueryRejection::InvalidResponse;
        match (&data.query, &mut data.task) {
            (Query::Observe { .. }, Task::Observe { builder }) => {
                if !self.observation_unit(builder, cache)? {
                    return Ok(None);
                }
                // Work is consumed at completion. Move its assembled rows out;
                // no observable incomplete prefix is changed by this placeholder.
                let Task::Observe { builder } = std::mem::replace(
                    &mut data.task,
                    Task::Decision {
                        cursor: 0,
                        request: None,
                    },
                ) else {
                    return Err(QueryRejection::Invariant);
                };
                Ok(Some(QueryValue::Observation(Observation {
                    observation_version: crate::formats::OBSERVATION_VERSION,
                    decision_context: self
                        .permitted_decision_context(builder.view.viewer, builder.decision.as_ref()),
                    view: builder.view,
                    decision: builder.decision,
                })))
            }
            (Query::Decision { viewer }, Task::Decision { cursor, request }) => {
                if let Some(
                    PlayerRequest::EntryCopy { candidates, .. }
                    | PlayerRequest::EntryAttachment { candidates, .. }
                    | PlayerRequest::EntryReveal { candidates, .. },
                ) = request
                {
                    let objects = cache.entry_candidates(self, *viewer)?;
                    if let Some(&object) = objects.get(*cursor as usize) {
                        candidates.push(
                            self.handle_for(*viewer, object)
                                .map_err(|_| QueryRejection::Invariant)?,
                        );
                    }
                    *cursor = cursor.checked_add(1).ok_or(QueryRejection::Invariant)?;
                    return Ok((*cursor as usize >= objects.len())
                        .then(|| QueryValue::Decision(request.take())));
                }

                if let Some(PlayerRequest::OrderTriggers { candidates, .. }) = request {
                    let ids = cache.triggers(self);
                    if let Some(id) = ids.get(*cursor as usize) {
                        candidates.push(
                            self.visible_pending_trigger(*viewer, *id)
                                .map_err(|_| QueryRejection::Invariant)?,
                        );
                    }
                    *cursor = cursor.checked_add(1).ok_or(QueryRejection::Invariant)?;
                    return Ok((*cursor as usize >= ids.len())
                        .then(|| QueryValue::Decision(request.take())));
                }

                if let Some(PlayerRequest::Modes { candidates, .. }) = request {
                    let Some(ids) = cache.modes(self)? else {
                        return Err(QueryRejection::Invariant);
                    };
                    if let Some(id) = ids.get(*cursor as usize) {
                        candidates.push(*id);
                    }
                    *cursor = cursor.checked_add(1).ok_or(QueryRejection::Invariant)?;
                    return Ok((*cursor as usize >= ids.len())
                        .then(|| QueryValue::Decision(request.take())));
                }
                if let Some(PlayerRequest::CounterPlacementEffect { candidates, .. }) = request {
                    let keys = cache.counter_placement(self);
                    if let Some(key) = keys.get(*cursor as usize) {
                        candidates.push(
                            self.visible_counter_candidate(*viewer, key)
                                .map_err(|_| QueryRejection::Invariant)?,
                        );
                    }
                    *cursor = cursor.checked_add(1).ok_or(QueryRejection::Invariant)?;
                    return Ok((*cursor as usize >= keys.len())
                        .then(|| QueryValue::Decision(request.take())));
                }
                if let Some(PlayerRequest::DamageEffect { candidates, .. }) = request {
                    let pairs = cache.damage(self)?;
                    if let Some(pair) = pairs.get(*cursor as usize) {
                        candidates.push(
                            self.visible_damage_effect_candidate(*viewer, pair)
                                .map_err(|_| QueryRejection::Invariant)?,
                        );
                    }
                    *cursor = cursor.checked_add(1).ok_or(QueryRejection::Invariant)?;
                    return Ok((*cursor as usize >= pairs.len())
                        .then(|| QueryValue::Decision(request.take())));
                }
                if let Some(PlayerRequest::EntityTargets { candidates, .. }) = request {
                    let cursor_index = *cursor as usize;
                    let target = if cursor_index < 4 {
                        self.targetable_player(PlayerId(cursor_index as u8))
                            .then_some(TargetEntity::Player {
                                player: PlayerId(cursor_index as u8),
                            })
                    } else {
                        cache
                            .next_object(self)
                            .filter(|o| {
                                self.top().is_ok_and(|e| {
                                    self.entity_target_legal(
                                        TargetEntity::Object { object: o.id },
                                        e,
                                    )
                                })
                            })
                            .map(|o| TargetEntity::Object { object: o.id })
                    };
                    if let Some(target) = target {
                        candidates.push(
                            self.player_entity(*viewer, target)
                                .map_err(|_| QueryRejection::Invariant)?,
                        );
                    }
                    *cursor = cursor.checked_add(1).ok_or(QueryRejection::Invariant)?;
                    return Ok((*cursor as usize >= 4 + self.state.rules.objects.len())
                        .then(|| QueryValue::Decision(request.take())));
                }

                if let Some(PlayerRequest::BottomOpening { candidates, .. }) = request {
                    if let Some(o) = cache.next_object(self)
                        && o.zone == (Zone::Hand { owner: *viewer })
                    {
                        candidates.push(
                            self.handle_for(*viewer, o.id)
                                .map_err(|_| QueryRejection::Invariant)?,
                        );
                    }
                    *cursor = cursor.checked_add(1).ok_or(QueryRejection::Invariant)?;
                    return Ok((*cursor as usize >= self.state.rules.objects.len())
                        .then(|| QueryValue::Decision(request.take())));
                }
                if !matches!(request, Some(PlayerRequest::Targets { .. })) {
                    return Ok(Some(QueryValue::Decision(request.take())));
                }
                if let Some(object) = cache.next_object(self)
                    && self
                        .top()
                        .is_ok_and(|entry| self.object_target_legal(entry, object.id))
                {
                    let handle = self
                        .handle_for(*viewer, object.id)
                        .map_err(|_| QueryRejection::Invariant)?;
                    if let Some(PlayerRequest::Targets { candidates, .. }) = request {
                        candidates.push(handle);
                    }
                }
                *cursor = cursor.checked_add(1).ok_or(QueryRejection::Invariant)?;
                Ok((*cursor as usize >= self.state.rules.objects.len())
                    .then(|| QueryValue::Decision(request.take())))
            }
            (
                Query::Check {
                    viewer,
                    token,
                    response,
                },
                Task::Check {
                    header_checked,
                    converted,
                    converted_entities,
                    converted_triggers,
                    value,
                },
            ) => {
                if !*header_checked {
                    self.validate_player_token(*viewer, token)
                        .map_err(invalid)?;
                    let timing_valid = match (&self.state.rules.frame, response) {
                        (Frame::Setup, PlayerResponse::Mulligan { .. }) => true,
                        (Frame::Setup, PlayerResponse::OpeningPlacement { .. }) => true,
                        (Frame::Setup, PlayerResponse::BottomOpening { objects }) => {
                            objects.len() <= 7
                        }
                        (
                            Frame::EntityTargets { .. },
                            PlayerResponse::EntityTargets { entities },
                        ) => {
                            let p = self.program().map_err(|e| invalid(e.into()))?;
                            entities.len() == p.targets.len()
                        }

                        (
                            Frame::ManaAbility { .. } | Frame::SpendMana { .. },
                            PlayerResponse::SpendManaAllocated {
                                spending,
                                restricted,
                            },
                        ) => {
                            spending.len() <= 6
                                && restricted.len() <= restricted_mana::MAX_RESTRICTED_BUCKETS
                        }
                        (
                            Frame::ManaAbility { .. },
                            PlayerResponse::ActivateMana { .. }
                            | PlayerResponse::FinishMana { .. }
                            | PlayerResponse::SpendMana { .. }
                            | PlayerResponse::ManaCombination { .. }
                            | PlayerResponse::ManaColor { .. },
                        ) => true,
                        (
                            Frame::ResolutionPayment { .. },
                            PlayerResponse::OptionalEffect { .. }
                            | PlayerResponse::ActivateMana { .. }
                            | PlayerResponse::FinishMana { .. }
                            | PlayerResponse::SpendMana { .. },
                        ) => true,
                        (
                            Frame::ManaSourceChoice {
                                scope: ManaSourceScope::Mixed,
                                ..
                            },
                            PlayerResponse::ChooseManaAbility { .. },
                        ) => true,
                        (
                            Frame::ManaSourceChoice { scope, .. },
                            PlayerResponse::ManaColor { .. },
                        ) => *scope != ManaSourceScope::Mixed,
                        (Frame::GraveyardGroup { .. }, PlayerResponse::OptionalEffect { .. }) => {
                            true
                        }
                        (
                            Frame::LandEntry { .. },
                            PlayerResponse::OptionalEffect { .. }
                            | PlayerResponse::EntryCopy { .. }
                            | PlayerResponse::EntryAttachment { .. }
                            | PlayerResponse::EntryReveal { .. },
                        ) => true,

                        (Frame::AdditionalCosts { .. }, PlayerResponse::AdditionalCosts { .. }) => {
                            true
                        }
                        (Frame::HybridCost { .. }, PlayerResponse::HybridCost { .. }) => true,
                        (Frame::AnnounceX { .. }, PlayerResponse::AnnounceX { .. }) => true,
                        (
                            Frame::Modes { .. },
                            PlayerResponse::Modes { .. } | PlayerResponse::ModeSubset { .. },
                        ) => true,
                        (Frame::SacrificeCost { .. }, PlayerResponse::SacrificeCost { .. })
                        | (Frame::ForcedSacrifice { .. }, PlayerResponse::ForcedSacrifice { .. })
                        | (Frame::Kernel { .. }, PlayerResponse::OptionalEffect { .. }) => true,
                        (
                            Frame::TriggerTargets { .. },
                            PlayerResponse::EntityTargets { entities },
                        ) => {
                            let p = self.program().map_err(|e| invalid(e.into()))?;
                            entities.len() == p.targets.len()
                        }
                        (Frame::TriggerTargets { .. }, PlayerResponse::Targets { objects }) => {
                            objects.len()
                                == self.program().map_err(|e| invalid(e.into()))?.targets.len()
                        }
                        (Frame::TriggerOrder { .. }, PlayerResponse::OrderTriggers { order }) => {
                            order.len() == self.trigger_order_ids().len()
                        }
                        (
                            Frame::TriggerTargets { .. },
                            PlayerResponse::PlayerTargets { players },
                        ) => {
                            players.len()
                                == self.program().map_err(|e| invalid(e.into()))?.targets.len()
                        }
                        (Frame::StackTargets { .. }, PlayerResponse::StackTargets { .. })
                        | (Frame::PlayerTargets { .. }, PlayerResponse::PlayerTargets { .. })
                        | (
                            Frame::ResolutionChoice { .. },
                            PlayerResponse::OptionalEffect { .. }
                            | PlayerResponse::Sacrifice { .. }
                            | PlayerResponse::ResolveDiscard { .. },
                        )
                        | (Frame::LegendRule { .. }, PlayerResponse::LegendRule { .. })
                        | (
                            Frame::CounterPlacement { .. },
                            PlayerResponse::CounterPlacementEffect { .. },
                        )
                        | (Frame::DamageEffects { .. }, PlayerResponse::DamageEffect { .. })
                        | (Frame::SpendMana { .. }, PlayerResponse::SpendMana { .. })
                        | (
                            Frame::CommanderReturn { .. }
                            | Frame::GraveyardReplacement { .. }
                            | Frame::GraveyardGroup { .. },
                            PlayerResponse::CommanderReturn { .. },
                        )
                        | (Frame::Attackers, PlayerResponse::Attackers { .. })
                        | (Frame::Blockers { .. }, PlayerResponse::Blockers { .. })
                        | (
                            Frame::DamageChoice { .. },
                            PlayerResponse::Damage { .. } | PlayerResponse::TrampleDamage { .. },
                        )
                        | (Frame::Discard { .. }, PlayerResponse::Discard { .. }) => true,
                        (Frame::Targets { .. }, PlayerResponse::Targets { objects }) => {
                            Self::object_target_number_valid(
                                self.program().map_err(|e| invalid(e.into()))?,
                                objects.len(),
                            )
                        }
                        (
                            Frame::Priority,
                            PlayerResponse::ActivateMana { .. }
                            | PlayerResponse::ActivateAbility { .. }
                            | PlayerResponse::Cast { .. }
                            | PlayerResponse::Bestow { .. }
                            | PlayerResponse::PlayLand { .. }
                            | PlayerResponse::PlayBackLand { .. }
                            | PlayerResponse::Pass,
                        ) => true,
                        (
                            Frame::Mana { .. },
                            PlayerResponse::ActivateMana { .. } | PlayerResponse::FinishMana { .. },
                        ) => true,
                        _ => false,
                    };
                    if !timing_valid {
                        return Err(QueryRejection::InvalidResponse);
                    }
                    *header_checked = true;
                    return Ok(None);
                }
                if let PlayerResponse::BottomOpening { objects } = response
                    && converted.len() < objects.len()
                {
                    let object = self
                        .resolve_handle(*viewer, &objects[converted.len()])
                        .map_err(invalid)?;
                    if self.current(object).map_err(|e| invalid(e.into()))?.zone
                        != (Zone::Hand { owner: *viewer })
                    {
                        return Err(QueryRejection::InvalidResponse);
                    }
                    converted.push(object);
                    return Ok(None);
                }
                if let PlayerResponse::Targets { objects } = response
                    && converted.len() < objects.len()
                {
                    let id = self
                        .resolve_handle(*viewer, &objects[converted.len()])
                        .map_err(invalid)?;
                    if !self
                        .top()
                        .is_ok_and(|entry| self.object_target_legal(entry, id))
                    {
                        return Err(QueryRejection::InvalidResponse);
                    }
                    converted.push(id);
                    return Ok(None);
                }
                if let PlayerResponse::EntityTargets { entities } = response
                    && converted_entities.len() < entities.len()
                {
                    let target = self
                        .resolve_entity(*viewer, &entities[converted_entities.len()])
                        .map_err(invalid)?;
                    if !self
                        .top()
                        .is_ok_and(|e| self.entity_target_legal(target, e))
                    {
                        return Err(QueryRejection::InvalidResponse);
                    }
                    converted_entities.push(target);
                    return Ok(None);
                }
                if let PlayerResponse::OrderTriggers { order } = response
                    && converted_triggers.len() < order.len()
                {
                    converted_triggers.push(
                        self.resolve_trigger_handle(*viewer, &order[converted_triggers.len()])
                            .map_err(invalid)?,
                    );
                    return Ok(None);
                }

                if value.is_none() {
                    *value = Some(
                        if matches!(response, PlayerResponse::BottomOpening { .. }) {
                            Response::BottomOpening {
                                objects: converted.clone(),
                            }
                        } else if matches!(response, PlayerResponse::Targets { .. }) {
                            Response::Targets {
                                objects: converted.clone(),
                            }
                        } else if matches!(response, PlayerResponse::EntityTargets { .. }) {
                            Response::EntityTargets {
                                entities: converted_entities.clone(),
                            }
                        } else if matches!(response, PlayerResponse::OrderTriggers { .. }) {
                            Response::OrderTriggers {
                                order: converted_triggers.clone(),
                            }
                        } else {
                            self.convert_response(*viewer, response).map_err(invalid)?
                        },
                    );
                    return Ok(None);
                }
                let receipt = self
                    .check_response(
                        *viewer,
                        self.state.rules.revision,
                        value.take().ok_or(QueryRejection::Invariant)?,
                    )
                    .map_err(invalid)?;
                Ok(Some(QueryValue::Validated(receipt)))
            }
            (
                Query::Search {
                    viewer,
                    sampling_seed,
                },
                Task::Search {
                    builder,
                    observation,
                    history,
                    decks,
                },
            ) => {
                if observation.is_none() {
                    if self.observation_unit(builder, cache)? {
                        // Search remains incomplete here. Keep its historical
                        // builder scratch intact for exact saved-work recovery.
                        *observation = Some(Box::new(Observation {
                            observation_version: crate::formats::OBSERVATION_VERSION,
                            decision_context: self.permitted_decision_context(
                                builder.view.viewer,
                                builder.decision.as_ref(),
                            ),
                            view: builder.view.clone(),
                            decision: builder.decision.clone(),
                        }));
                    }
                    return Ok(None);
                }
                let permitted = &self.state.knowledge.visibility[viewer.0 as usize].history;
                if history.len() < permitted.len() {
                    history.push_stored(permitted[history.len()].clone());
                    return Ok(None);
                }
                if decks.len() < 4 {
                    let player = PlayerId(decks.len() as u8);
                    let mut definitions = BTreeMap::new();
                    for card in self
                        .prepared
                        .config
                        .cards
                        .iter()
                        .filter(|c| c.owner == player)
                    {
                        let count = definitions.entry(card.oracle_id.clone()).or_insert(0_u32);
                        *count = count.checked_add(1).ok_or(QueryRejection::Invariant)?;
                    }
                    decks.push(OpenDecklist {
                        player,
                        definitions,
                    });
                    return Ok(None);
                }
                Ok(Some(QueryValue::SearchContext(SearchContext {
                    context_version: crate::formats::SEARCH_CONTEXT_VERSION,
                    observation: *observation.take().ok_or(QueryRejection::Invariant)?,
                    permitted_history: std::mem::take(history),
                    open_decklists: std::mem::take(decks),
                    catalog_digest: self.prepared.catalog_digest.clone(),
                    profile_id: self.prepared.config.profile.clone(),
                    sampling_seed: sampling_seed.clone(),
                })))
            }
            _ => Err(QueryRejection::Invariant),
        }
    }
    pub fn save_work(&self, work: &Work) -> Result<String, String> {
        if !Arc::ptr_eq(&work.executor, &self.executor)
            || !Arc::ptr_eq(&work.state_token, &self.state_token)
        {
            return Err("stale or foreign work".into());
        }
        canonical(&SavedWork {
            work_version: crate::formats::WORK_VERSION,
            build_id: build_id().into(),
            catalog_digest: self.prepared.catalog_digest.clone(),
            state_sha256: self.state_digest()?,
            data: work.data.clone(),
        })
    }
    pub fn restore_work(&self, json: &str) -> Result<Work, String> {
        let saved: SavedWork = strict_read(json)?;
        if saved.work_version != crate::formats::WORK_VERSION
            || saved.build_id != build_id()
            || saved.catalog_digest != self.prepared.catalog_digest
            || saved.state_sha256 != self.state_digest()?
        {
            return Err("query recovery identity mismatch".into());
        }
        let mut work = self
            .start_work(saved.data.query.clone())
            .map_err(|_| "unsupported recovered query")?;
        let total_units = self.query_work_units(&saved.data.query)?;
        if saved.data.units_done >= total_units {
            return Err("invalid recovered query cursor".into());
        }
        for _ in 0..saved.data.units_done {
            work.data.units_done += 1;
            if self
                .query_unit(&mut work.data, &mut work.cache)
                .map_err(|_| "invalid query prefix")?
                .is_some()
            {
                return Err("saved work is already complete".into());
            }
        }
        if work.data != saved.data {
            return Err("saved scratch does not match recomputed query prefix".into());
        }
        Ok(work)
    }
}

impl RulesTxn {
    fn request_extra_units(&self, viewer: PlayerId) -> Result<u64, String> {
        Ok(match self.request_header(viewer)? {
            Some(
                PlayerRequest::EntryCopy { .. }
                | PlayerRequest::EntryAttachment { .. }
                | PlayerRequest::EntryReveal { .. },
            ) => self.entry_work_candidates(viewer)?.len() as u64,

            Some(PlayerRequest::OrderTriggers { .. }) => self.trigger_order_ids().len() as u64,
            Some(PlayerRequest::Modes { .. }) => {
                if let Some(Request::Modes { candidates, .. }) = self.modes_request()? {
                    candidates.len() as u64
                } else {
                    0
                }
            }
            Some(PlayerRequest::CounterPlacementEffect { .. }) => {
                self.counter_placement_candidates().len() as u64
            }
            Some(PlayerRequest::DamageEffect { .. }) => self
                .damage_effect_group()?
                .map_or(0, |(_, _, p)| p.len() as u64),

            _ => 0,
        })
    }
    fn entry_work_candidates(&self, viewer: PlayerId) -> Result<Vec<ObjectId>, String> {
        Ok(match self.land_entry_request()? {
            Some(
                Request::EntryCopy {
                    player, candidates, ..
                }
                | Request::EntryAttachment {
                    player, candidates, ..
                }
                | Request::EntryReveal {
                    player, candidates, ..
                },
            ) if viewer == player => candidates,
            _ => vec![],
        })
    }

    pub(crate) fn observation_work_units(&self, viewer: PlayerId) -> Result<u64, String> {
        4u64.checked_add(self.state.rules.objects.len() as u64)
            .and_then(|n| n.checked_add(self.state.rules.stack.len() as u64))
            .and_then(|n| n.checked_add(self.live_effect_keys().len() as u64))
            .and_then(|n| n.checked_add(self.request_extra_units(viewer).ok()?))
            .ok_or_else(|| "observation work resource limit".into())
    }
    // A saved prefix must fit this exact task's row/conversion count. History
    // cannot stand in for effect, decision-candidate or response-entry work.
    pub(crate) fn query_work_units(&self, query: &Query) -> Result<u64, String> {
        let body_units: Result<u64, String> = match query {
            Query::Observe { viewer } => self.observation_work_units(*viewer),
            Query::Decision { viewer } => Ok(match self.request_header(*viewer)? {
                Some(PlayerRequest::Targets { .. } | PlayerRequest::BottomOpening { .. }) => {
                    std::cmp::max(1, self.state.rules.objects.len() as u64)
                }
                Some(PlayerRequest::EntityTargets { .. }) => 4u64
                    .checked_add(self.state.rules.objects.len() as u64)
                    .ok_or("decision work resource limit")?,
                Some(
                    PlayerRequest::OrderTriggers { .. }
                    | PlayerRequest::EntryCopy { .. }
                    | PlayerRequest::EntryAttachment { .. }
                    | PlayerRequest::EntryReveal { .. }
                    | PlayerRequest::Modes { .. }
                    | PlayerRequest::CounterPlacementEffect { .. }
                    | PlayerRequest::DamageEffect { .. },
                ) => std::cmp::max(1, self.request_extra_units(*viewer)?),
                _ => 1,
            }),
            Query::Check { response, .. } => {
                let entries = match response {
                    PlayerResponse::Targets { objects }
                    | PlayerResponse::BottomOpening { objects } => objects.len(),
                    PlayerResponse::ModeSubset { modes } => modes.len(),
                    PlayerResponse::EntityTargets { entities } => entities.len(),
                    PlayerResponse::OrderTriggers { order } => order.len(),

                    _ => 0,
                };
                3u64.checked_add(entries as u64)
                    .ok_or_else(|| "response work resource limit".into())
            }
            Query::Search { viewer, .. } => self
                .observation_work_units(*viewer)?
                .checked_add(
                    self.state
                        .knowledge
                        .visibility
                        .get(viewer.0 as usize)
                        .ok_or("invalid context viewer")?
                        .history
                        .len() as u64,
                )
                .and_then(|n| n.checked_add(5))
                .ok_or_else(|| "context work resource limit".into()),
        };
        body_units?
            .checked_add(1)
            .ok_or_else(|| "query initialization work resource limit".into())
    }
}
