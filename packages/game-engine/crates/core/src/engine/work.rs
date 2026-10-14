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
    pub(super) fn consume(&mut self) -> bool {
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
pub struct Work {
    executor: Arc<()>,
    state_token: Arc<()>,
    data: WorkData,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "query", rename_all = "snake_case", deny_unknown_fields)]
enum Query {
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
struct ObservationWork {
    cursor: u32,
    view: PermittedView,
    decision: Option<PlayerRequest>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "task", rename_all = "snake_case", deny_unknown_fields)]
enum Task {
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
        value: Option<Response>,
    },
    Search {
        builder: ObservationWork,
        observation: Option<Box<Observation>>,
        history: Vec<PermittedView>,
        decks: Vec<OpenDecklist>,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkData {
    query: Query,
    task: Task,
    units_done: u64,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SavedWork {
    work_version: u32,
    build_id: String,
    catalog_digest: String,
    state_sha256: String,
    data: WorkData,
}

impl Game {
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
    fn start_work(&self, query: Query) -> Result<Work, QueryRejection> {
        let viewer = match &query {
            Query::Observe { viewer }
            | Query::Decision { viewer }
            | Query::Check { viewer, .. }
            | Query::Search { viewer, .. } => *viewer,
        };
        if viewer.0 >= 4 {
            return Err(QueryRejection::InvalidViewer);
        }
        if !self.state.pending_concessions.is_empty() {
            return Err(QueryRejection::UnsupportedBoundary);
        }
        if let Query::Search { sampling_seed, .. } = &query
            && (!matches!(self.state.frame, Frame::Priority)
                || self.state.pending.is_some()
                || !self.is_live(viewer)
                || sampling_seed.is_empty()
                || sampling_seed.len() > 128)
        {
            return Err(QueryRejection::UnsupportedBoundary);
        }
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
                value: None,
            },
            Query::Search { .. } => Task::Search {
                builder: observe()?,
                observation: None,
                history: vec![],
                decks: vec![],
            },
        };
        Ok(Work {
            executor: self.executor.clone(),
            state_token: self.state_token.clone(),
            data: WorkData {
                query,
                task,
                units_done: 0,
            },
        })
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
            match self.query_unit(&mut work.data) {
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
    ) -> Result<Option<Observation>, QueryRejection> {
        let cursor = b.cursor as usize;
        let viewer = b.view.viewer;
        if cursor < 4 {
            b.view.players.push(
                self.visible_player(PlayerId(cursor as u8))
                    .map_err(|_| QueryRejection::Invariant)?,
            );
        } else if cursor < 4 + self.state.objects.len() {
            if let Some(object) = self.state.objects.values().nth(cursor - 4)
                && let Some(visible) = self
                    .visible_object(viewer, object)
                    .map_err(|_| QueryRejection::Invariant)?
            {
                if let Some(PlayerRequest::Targets { candidates, .. }) = &mut b.decision
                    && self.target_legal(object.id)
                {
                    candidates.push(visible.handle.clone());
                }
                b.view.objects.push(visible);
            }
        } else {
            b.view.stack.push(
                self.visible_stack(viewer, cursor - 4 - self.state.objects.len())
                    .map_err(|_| QueryRejection::Invariant)?,
            );
        }
        b.cursor = b.cursor.checked_add(1).ok_or(QueryRejection::Invariant)?;
        if b.cursor as usize >= 4 + self.state.objects.len() + self.state.stack.len() {
            b.view.objects.sort_by(|a, b| a.handle.0.cmp(&b.handle.0));
            return Ok(Some(Observation {
                observation_version: 5,
                view: b.view.clone(),
                decision: b.decision.clone(),
            }));
        }
        Ok(None)
    }
    fn query_unit(&self, data: &mut WorkData) -> Result<Option<QueryValue>, QueryRejection> {
        let invalid = |_: String| QueryRejection::InvalidResponse;
        match (&data.query, &mut data.task) {
            (Query::Observe { .. }, Task::Observe { builder }) => {
                Ok(self.observation_unit(builder)?.map(QueryValue::Observation))
            }
            (Query::Decision { viewer }, Task::Decision { cursor, request }) => {
                if !matches!(request, Some(PlayerRequest::Targets { .. })) {
                    return Ok(Some(QueryValue::Decision(request.clone())));
                }
                if let Some(object) = self.state.objects.values().nth(*cursor as usize)
                    && self.target_legal(object.id)
                {
                    let handle = self
                        .handle_for(*viewer, object.id)
                        .map_err(|_| QueryRejection::Invariant)?;
                    if let Some(PlayerRequest::Targets { candidates, .. }) = request {
                        candidates.push(handle);
                    }
                }
                *cursor = cursor.checked_add(1).ok_or(QueryRejection::Invariant)?;
                Ok((*cursor as usize >= self.state.objects.len())
                    .then(|| QueryValue::Decision(request.clone())))
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
                    value,
                },
            ) => {
                if !*header_checked {
                    self.validate_player_token(*viewer, token)
                        .map_err(invalid)?;
                    let timing_valid = match (&self.state.frame, response) {
                        (Frame::Targets { .. }, PlayerResponse::Targets { objects }) => {
                            objects.len() == self.program().map_err(invalid)?.targets.len()
                        }
                        (
                            Frame::Priority,
                            PlayerResponse::ActivateMana { .. }
                            | PlayerResponse::Cast { .. }
                            | PlayerResponse::PlayLand { .. }
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
                if let PlayerResponse::Targets { objects } = response
                    && converted.len() < objects.len()
                {
                    let id = self
                        .resolve_handle(*viewer, &objects[converted.len()])
                        .map_err(invalid)?;
                    if !self.target_legal(id) {
                        return Err(QueryRejection::InvalidResponse);
                    }
                    converted.push(id);
                    return Ok(None);
                }
                if value.is_none() {
                    *value = Some(if matches!(response, PlayerResponse::Targets { .. }) {
                        Response::Targets {
                            objects: converted.clone(),
                        }
                    } else {
                        self.convert_response(*viewer, response).map_err(invalid)?
                    });
                    return Ok(None);
                }
                let receipt = self
                    .check_response(
                        *viewer,
                        self.state.revision,
                        value.clone().ok_or(QueryRejection::Invariant)?,
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
                    *observation = self.observation_unit(builder)?.map(Box::new);
                    return Ok(None);
                }
                let permitted = &self.state.visibility[viewer.0 as usize].history;
                if history.len() < permitted.len() {
                    history.push(permitted[history.len()].clone());
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
                    context_version: 5,
                    observation: *observation.clone().ok_or(QueryRejection::Invariant)?,
                    permitted_history: history.clone(),
                    open_decklists: decks.clone(),
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
            work_version: 4,
            build_id: BUILD_ID.into(),
            catalog_digest: self.prepared.catalog_digest.clone(),
            state_sha256: self.state_digest()?,
            data: work.data.clone(),
        })
    }
    pub fn restore_work(&self, json: &str) -> Result<Work, String> {
        let saved: SavedWork = strict_read(json)?;
        if saved.work_version != 4
            || saved.build_id != BUILD_ID
            || saved.catalog_digest != self.prepared.catalog_digest
            || saved.state_sha256 != self.state_digest()?
        {
            return Err("query recovery identity mismatch".into());
        }
        let mut work = self
            .start_work(saved.data.query.clone())
            .map_err(|_| "unsupported recovered query")?;
        let max_units = 16_u64
            .checked_add(
                u64::try_from(self.state.stack.len())
                    .map_err(|_| "stack work bound resource limit")?,
            )
            .ok_or("stack work bound resource limit")?
            .checked_add(self.state.objects.len() as u64)
            .and_then(|n| {
                n.checked_add(
                    self.state
                        .visibility
                        .iter()
                        .map(|v| v.history.len() as u64)
                        .max()
                        .unwrap_or(0),
                )
            })
            .ok_or("work size limit")?;
        if saved.data.units_done > max_units {
            return Err("invalid recovered query cursor".into());
        }
        for _ in 0..saved.data.units_done {
            work.data.units_done += 1;
            if self
                .query_unit(&mut work.data)
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
