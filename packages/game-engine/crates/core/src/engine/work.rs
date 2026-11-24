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
    Decision(Option<Decision<Viewer>>),
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
        answer: Answer,
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
    pub decision: Option<Decision<Viewer>>,
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
    Decision,
    Check,
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
            && let Ok(decision) = self.viewer_decision(viewer)
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
        answer: Answer,
        budget: Budget,
    ) -> QueryResult {
        self.begin_query(
            Query::Check {
                viewer,
                token,
                answer,
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
                    .viewer_decision(viewer)
                    .map_err(|_| QueryRejection::Invariant)?,
            })
        };
        let task = match query {
            Query::Observe { .. } => Task::Observe {
                builder: observe()?,
            },
            Query::Decision { .. } => Task::Decision,
            Query::Check { .. } => Task::Check,
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
        let effect_count = live_effect_count;
        b.cursor = b.cursor.checked_add(1).ok_or(QueryRejection::Invariant)?;
        if cursor >= base && cursor < base + effect_count {
            let effect = self.visible_effect(viewer, &cache.effects(self)[cursor - base]);
            b.view
                .effects
                .push(effect.map_err(|_| QueryRejection::Invariant)?);
        }

        let extra = effect_count;
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
        match (&data.query, &mut data.task) {
            (Query::Observe { .. }, Task::Observe { builder }) => {
                if !self.observation_unit(builder, cache)? {
                    return Ok(None);
                }
                // Work is consumed at completion. Move its assembled rows out;
                // no observable incomplete prefix is changed by this placeholder.
                let Task::Observe { builder } = std::mem::replace(&mut data.task, Task::Decision)
                else {
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
            (Query::Decision { viewer }, Task::Decision) => Ok(Some(QueryValue::Decision(
                self.viewer_decision(*viewer)
                    .map_err(|_| QueryRejection::Invariant)?,
            ))),
            (
                Query::Check {
                    viewer,
                    token,
                    answer,
                },
                Task::Check,
            ) => {
                self.validate_player_token(*viewer, token)
                    .map_err(|_| QueryRejection::InvalidResponse)?;
                let receipt = self
                    .check_response(*viewer, self.state.rules.revision, answer.clone())
                    .map_err(|_| QueryRejection::InvalidResponse)?;
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
    pub(crate) fn observation_work_units(&self, _viewer: PlayerId) -> Result<u64, String> {
        4u64.checked_add(self.state.rules.objects.len() as u64)
            .and_then(|n| n.checked_add(self.state.rules.stack.len() as u64))
            .and_then(|n| n.checked_add(self.live_effect_keys().len() as u64))
            .ok_or_else(|| "observation work resource limit".into())
    }
    // A saved prefix must fit this exact task's row/conversion count. History
    // cannot stand in for effect, decision-candidate or response-entry work.
    pub(crate) fn query_work_units(&self, query: &Query) -> Result<u64, String> {
        let body_units: Result<u64, String> = match query {
            Query::Observe { viewer } => self.observation_work_units(*viewer),
            Query::Decision { .. } | Query::Check { .. } => Ok(1),
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
