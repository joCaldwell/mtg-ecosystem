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
pub struct Work {
    executor: Arc<()>,
    state_token: Arc<()>,
    data: WorkData,
    cache: Box<WorkCache>,
}

// Pure derived accelerators live only in opaque, source/state-scoped Work.
// They never enter SavedWork or semantic State. Initialization remains lazy:
// Budget0 does not enumerate anything. The first existing unit computes each
// domain exactly as before; later units reuse it without changing row counts.
#[derive(Default)]
struct WorkCache {
    last_object: Option<CardId>,
    effects: Option<Vec<EffectKey>>,
    triggers: Option<Vec<TriggerId>>,
    modes: Option<Option<Vec<u32>>>,
    damage: Option<Vec<DamageEffectCandidate>>,
    allocation: Option<Option<Vec<DamageAllocationCandidate>>>,
}
impl WorkCache {
    fn next_object<'a>(&mut self, game: &'a Game) -> Option<&'a Object> {
        use std::ops::Bound::{Excluded, Unbounded};
        let next = match self.last_object {
            None => game.state.objects.first_key_value(),
            Some(last) => game.state.objects.range((Excluded(last), Unbounded)).next(),
        };
        if let Some((key, _)) = next {
            self.last_object = Some(*key);
        }
        next.map(|(_, object)| object)
    }
    fn effects(&mut self, game: &Game) -> &[EffectKey] {
        self.effects.get_or_insert_with(|| game.live_effect_keys())
    }
    fn triggers(&mut self, game: &Game) -> &[TriggerId] {
        self.triggers
            .get_or_insert_with(|| game.trigger_order_ids())
    }
    fn modes(&mut self, game: &Game) -> Result<Option<&[u32]>, QueryRejection> {
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
    fn damage(&mut self, game: &Game) -> Result<&[DamageEffectCandidate], QueryRejection> {
        if self.damage.is_none() {
            self.damage = Some(
                game.damage_effect_group()
                    .map_err(|_| QueryRejection::Invariant)?
                    .map_or(vec![], |(_, _, pairs)| pairs),
            );
        }
        Ok(self.damage.as_deref().unwrap_or_default())
    }
    fn allocation(
        &mut self,
        game: &Game,
    ) -> Result<Option<&[DamageAllocationCandidate]>, QueryRejection> {
        if self.allocation.is_none() {
            self.allocation = Some(
                match game
                    .damage_allocation_request()
                    .map_err(|_| QueryRejection::Invariant)?
                {
                    Some(Request::DamageAllocation { candidates, .. }) => Some(candidates),
                    _ => None,
                },
            );
        }
        Ok(self.allocation.as_ref().and_then(|ids| ids.as_deref()))
    }
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
        converted_entities: Vec<TargetEntity>,
        converted_triggers: Vec<TriggerId>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        converted_allocations: Vec<(DamageId, Quantity)>,
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
        if self.setup_automatic() {
            return Err(QueryRejection::UnavailableDuringSetup);
        }
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
                converted_entities: vec![],
                converted_triggers: vec![],
                converted_allocations: vec![],
                value: None,
            },
            Query::Search { .. } => Task::Search {
                builder: observe()?,
                observation: None,
                history: PermittedHistory::default(),
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
            cache: Box::default(),
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
    ) -> Result<Option<Observation>, QueryRejection> {
        let cursor = b.cursor as usize;
        let viewer = b.view.viewer;
        if cursor < 4 {
            if let Some(PlayerRequest::EntityTargets { candidates, .. }) = &mut b.decision
                && self.is_live(PlayerId(cursor as u8))
            {
                candidates.push(PlayerTargetEntity::Player {
                    player: PlayerId(cursor as u8),
                });
            }
            b.view.players.push(
                self.visible_player(PlayerId(cursor as u8))
                    .map_err(|_| QueryRejection::Invariant)?,
            );
        } else if cursor < 4 + self.state.objects.len() {
            if let Some(object) = cache.next_object(self)
                && let Some(visible) = self
                    .visible_object(viewer, object)
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
                    && self
                        .top()
                        .is_ok_and(|entry| self.object_target_legal(entry, object.id))
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
        } else if cursor < 4 + self.state.objects.len() + self.state.stack.len() {
            b.view.stack.push(
                self.visible_stack(viewer, cursor - 4 - self.state.objects.len())
                    .map_err(|_| QueryRejection::Invariant)?,
            );
        }
        if let Some(PlayerRequest::OrderTriggers { candidates, .. }) = &mut b.decision {
            let offset =
                4 + self.state.objects.len() + self.state.stack.len() + cache.effects(self).len();
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
        let base = 4 + self.state.objects.len() + self.state.stack.len();
        let effect_count = cache.effects(self).len();
        if cursor >= base && cursor < base + effect_count {
            b.view.effects.push(
                self.visible_effect(viewer, &cache.effects(self)[cursor - base])
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
        if let Some(PlayerRequest::DamageAllocation { candidates, .. }) = &mut b.decision
            && cursor >= base + effect_count
            && let Some(ids) = cache.allocation(self)?
            && let Some(c) = ids.get(cursor - base - effect_count)
        {
            candidates.push(
                self.visible_allocation_candidate(viewer, c)
                    .map_err(|_| QueryRejection::Invariant)?,
            );
        }
        let extra = effect_count
            + if matches!(b.decision, Some(PlayerRequest::DamageAllocation { .. })) {
                cache.allocation(self)?.map_or(0, |ids| ids.len())
            } else if matches!(b.decision, Some(PlayerRequest::Modes { .. })) {
                cache.modes(self)?.map_or(0, |ids| ids.len())
            } else if matches!(b.decision, Some(PlayerRequest::DamageEffect { .. })) {
                cache.damage(self)?.len()
            } else if matches!(b.decision, Some(PlayerRequest::OrderTriggers { .. })) {
                cache.triggers(self).len()
            } else {
                0
            };
        if b.cursor as usize >= 4 + self.state.objects.len() + self.state.stack.len() + extra {
            b.view.objects.sort_by(|a, b| a.handle.0.cmp(&b.handle.0));
            return Ok(Some(Observation {
                observation_version: 16,
                view: b.view.clone(),
                decision: b.decision.clone(),
            }));
        }
        Ok(None)
    }
    fn query_unit(
        &self,
        data: &mut WorkData,
        cache: &mut WorkCache,
    ) -> Result<Option<QueryValue>, QueryRejection> {
        let invalid = |_: String| QueryRejection::InvalidResponse;
        match (&data.query, &mut data.task) {
            (Query::Observe { .. }, Task::Observe { builder }) => Ok(self
                .observation_unit(builder, cache)?
                .map(QueryValue::Observation)),
            (Query::Decision { viewer }, Task::Decision { cursor, request }) => {
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
                        .then(|| QueryValue::Decision(request.clone())));
                }
                if let Some(PlayerRequest::DamageAllocation { candidates, .. }) = request {
                    let Some(ids) = cache.allocation(self)? else {
                        return Err(QueryRejection::Invariant);
                    };
                    if let Some(candidate) = ids.get(*cursor as usize) {
                        candidates.push(
                            self.visible_allocation_candidate(*viewer, candidate)
                                .map_err(|_| QueryRejection::Invariant)?,
                        )
                    }
                    *cursor = cursor.checked_add(1).ok_or(QueryRejection::Invariant)?;
                    return Ok((*cursor as usize >= ids.len())
                        .then(|| QueryValue::Decision(request.clone())));
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
                        .then(|| QueryValue::Decision(request.clone())));
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
                        .then(|| QueryValue::Decision(request.clone())));
                }
                if let Some(PlayerRequest::EntityTargets { candidates, .. }) = request {
                    let cursor_index = *cursor as usize;
                    let target = if cursor_index < 4 {
                        self.is_live(PlayerId(cursor_index as u8))
                            .then_some(TargetEntity::Player {
                                player: PlayerId(cursor_index as u8),
                            })
                    } else {
                        cache
                            .next_object(self)
                            .filter(|o| self.top().is_ok_and(|e| self.object_target_legal(e, o.id)))
                            .map(|o| TargetEntity::Object { object: o.id })
                    };
                    if let Some(target) = target {
                        candidates.push(
                            self.player_entity(*viewer, target)
                                .map_err(|_| QueryRejection::Invariant)?,
                        );
                    }
                    *cursor = cursor.checked_add(1).ok_or(QueryRejection::Invariant)?;
                    return Ok((*cursor as usize >= 4 + self.state.objects.len())
                        .then(|| QueryValue::Decision(request.clone())));
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
                    return Ok((*cursor as usize >= self.state.objects.len())
                        .then(|| QueryValue::Decision(request.clone())));
                }
                if !matches!(request, Some(PlayerRequest::Targets { .. })) {
                    return Ok(Some(QueryValue::Decision(request.clone())));
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
                    converted_entities,
                    converted_triggers,
                    converted_allocations,
                    value,
                },
            ) => {
                if !*header_checked {
                    self.validate_player_token(*viewer, token)
                        .map_err(invalid)?;
                    let timing_valid = match (&self.state.frame, response) {
                        (Frame::Setup, PlayerResponse::Mulligan { .. }) => true,
                        (Frame::Setup, PlayerResponse::BottomOpening { objects }) => {
                            objects.len() <= 7
                        }
                        (
                            Frame::EntityTargets { .. },
                            PlayerResponse::EntityTargets { entities },
                        ) => entities.len() == self.program().map_err(invalid)?.targets.len(),
                        (
                            Frame::DamageAllocation { .. },
                            PlayerResponse::DamageAllocation { allocations },
                        ) => allocations.len() <= self.damage_limit(),
                        (
                            Frame::ManaAbility { .. },
                            PlayerResponse::ActivateMana { .. }
                            | PlayerResponse::FinishMana { .. }
                            | PlayerResponse::SpendMana { .. }
                            | PlayerResponse::ManaColor { .. },
                        ) => true,
                        (Frame::AnnounceX { .. }, PlayerResponse::AnnounceX { .. }) => true,
                        (Frame::Modes { .. }, PlayerResponse::Modes { .. }) => true,
                        (Frame::SacrificeCost { .. }, PlayerResponse::SacrificeCost { .. })
                        | (Frame::ForcedSacrifice { .. }, PlayerResponse::ForcedSacrifice { .. })
                        | (Frame::OptionalDamage { .. }, PlayerResponse::OptionalEffect { .. }) => {
                            true
                        }
                        (
                            Frame::TriggerTargets { .. },
                            PlayerResponse::EntityTargets { entities },
                        ) => entities.len() == self.program().map_err(invalid)?.targets.len(),
                        (Frame::TriggerTargets { .. }, PlayerResponse::Targets { objects }) => {
                            objects.len() == self.program().map_err(invalid)?.targets.len()
                        }
                        (Frame::TriggerOrder { .. }, PlayerResponse::OrderTriggers { order }) => {
                            order.len() == self.trigger_order_ids().len()
                        }
                        (
                            Frame::TriggerTargets { .. },
                            PlayerResponse::PlayerTargets { players },
                        ) => players.len() == self.program().map_err(invalid)?.targets.len(),
                        (Frame::StackTargets { .. }, PlayerResponse::StackTargets { .. })
                        | (Frame::PlayerTargets { .. }, PlayerResponse::PlayerTargets { .. })
                        | (
                            Frame::ResolutionChoice { .. },
                            PlayerResponse::SearchLands { .. }
                            | PlayerResponse::OptionalEffect { .. }
                            | PlayerResponse::OptionalRecursion { .. }
                            | PlayerResponse::Sacrifice { .. }
                            | PlayerResponse::ResolveDiscard { .. },
                        )
                        | (Frame::LegendRule { .. }, PlayerResponse::LegendRule { .. })
                        | (Frame::DamageEffects { .. }, PlayerResponse::DamageEffect { .. })
                        | (Frame::SpendMana { .. }, PlayerResponse::SpendMana { .. })
                        | (Frame::CommanderReturn { .. }, PlayerResponse::CommanderReturn { .. })
                        | (Frame::Attackers, PlayerResponse::Attackers { .. })
                        | (Frame::Blockers { .. }, PlayerResponse::Blockers { .. })
                        | (
                            Frame::DamageChoice { .. },
                            PlayerResponse::Damage { .. } | PlayerResponse::TrampleDamage { .. },
                        )
                        | (Frame::Discard { .. }, PlayerResponse::Discard { .. }) => true,
                        (Frame::Targets { .. }, PlayerResponse::Targets { objects }) => {
                            objects.len() == self.program().map_err(invalid)?.targets.len()
                        }
                        (
                            Frame::Priority,
                            PlayerResponse::ActivateMana { .. }
                            | PlayerResponse::ActivateAbility { .. }
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
                if let PlayerResponse::BottomOpening { objects } = response
                    && converted.len() < objects.len()
                {
                    let object = self
                        .resolve_handle(*viewer, &objects[converted.len()])
                        .map_err(invalid)?;
                    if self.current(object).map_err(invalid)?.zone
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
                        .is_ok_and(|e| self.entity_target_legal(target, e.controller))
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
                if let PlayerResponse::DamageAllocation { allocations } = response
                    && converted_allocations.len() < allocations.len()
                {
                    let (handle, amount) = &allocations[converted_allocations.len()];
                    let id = self
                        .resolve_damage_handle(*viewer, handle)
                        .map_err(invalid)?;
                    converted_allocations.push((id, amount.clone()));
                    return Ok(None);
                }
                if value.is_none() {
                    *value = Some(
                        if matches!(response, PlayerResponse::DamageAllocation { .. }) {
                            Response::DamageAllocation {
                                allocations: converted_allocations.clone(),
                            }
                        } else if matches!(response, PlayerResponse::BottomOpening { .. }) {
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
                    *observation = self.observation_unit(builder, cache)?.map(Box::new);
                    return Ok(None);
                }
                let permitted = &self.state.visibility[viewer.0 as usize].history;
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
                    context_version: 16,
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
            work_version: 15,
            build_id: BUILD_ID.into(),
            catalog_digest: self.prepared.catalog_digest.clone(),
            state_sha256: self.state_digest()?,
            data: work.data.clone(),
        })
    }
    pub fn restore_work(&self, json: &str) -> Result<Work, String> {
        let saved: SavedWork = strict_read(json)?;
        if saved.work_version != 15
            || saved.build_id != BUILD_ID
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

impl Game {
    fn request_extra_units(&self, viewer: PlayerId) -> Result<u64, String> {
        Ok(match self.request_header(viewer)? {
            Some(PlayerRequest::OrderTriggers { .. }) => self.trigger_order_ids().len() as u64,
            Some(PlayerRequest::Modes { .. }) => {
                if let Some(Request::Modes { candidates, .. }) = self.modes_request()? {
                    candidates.len() as u64
                } else {
                    0
                }
            }
            Some(PlayerRequest::DamageEffect { .. }) => self
                .damage_effect_group()?
                .map_or(0, |(_, _, p)| p.len() as u64),
            Some(PlayerRequest::DamageAllocation { .. }) => {
                if let Some(Request::DamageAllocation { candidates, .. }) =
                    self.damage_allocation_request()?
                {
                    candidates.len() as u64
                } else {
                    0
                }
            }
            _ => 0,
        })
    }
    fn observation_work_units(&self, viewer: PlayerId) -> Result<u64, String> {
        4u64.checked_add(self.state.objects.len() as u64)
            .and_then(|n| n.checked_add(self.state.stack.len() as u64))
            .and_then(|n| n.checked_add(self.live_effect_keys().len() as u64))
            .and_then(|n| n.checked_add(self.request_extra_units(viewer).ok()?))
            .ok_or_else(|| "observation work resource limit".into())
    }
    // A saved prefix must fit this exact task's row/conversion count. History
    // cannot stand in for effect, decision-candidate or response-entry work.
    fn query_work_units(&self, query: &Query) -> Result<u64, String> {
        match query {
            Query::Observe { viewer } => self.observation_work_units(*viewer),
            Query::Decision { viewer } => Ok(match self.request_header(*viewer)? {
                Some(PlayerRequest::Targets { .. } | PlayerRequest::BottomOpening { .. }) => {
                    std::cmp::max(1, self.state.objects.len() as u64)
                }
                Some(PlayerRequest::EntityTargets { .. }) => 4u64
                    .checked_add(self.state.objects.len() as u64)
                    .ok_or("decision work resource limit")?,
                Some(
                    PlayerRequest::OrderTriggers { .. }
                    | PlayerRequest::Modes { .. }
                    | PlayerRequest::DamageEffect { .. }
                    | PlayerRequest::DamageAllocation { .. },
                ) => std::cmp::max(1, self.request_extra_units(*viewer)?),
                _ => 1,
            }),
            Query::Check { response, .. } => {
                let entries = match response {
                    PlayerResponse::Targets { objects }
                    | PlayerResponse::BottomOpening { objects } => objects.len(),
                    PlayerResponse::EntityTargets { entities } => entities.len(),
                    PlayerResponse::OrderTriggers { order } => order.len(),
                    PlayerResponse::DamageAllocation { allocations } => allocations.len(),
                    _ => 0,
                };
                3u64.checked_add(entries as u64)
                    .ok_or_else(|| "response work resource limit".into())
            }
            Query::Search { viewer, .. } => self
                .observation_work_units(*viewer)?
                .checked_add(
                    self.state
                        .visibility
                        .get(viewer.0 as usize)
                        .ok_or("invalid context viewer")?
                        .history
                        .len() as u64,
                )
                .and_then(|n| n.checked_add(5))
                .ok_or_else(|| "context work resource limit".into()),
        }
    }
}

#[cfg(test)]
#[path = "work_optimization_tests.rs"]
pub(super) mod optimization_tests;
