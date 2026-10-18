//! Frozen pre-optimization query algorithm: copied before Work caches existed.
//! Kept as an independent exact unit/cursor/serialization oracle.
use super::*;
impl Game {
    fn reference_observation_unit(
        &self,
        b: &mut ObservationWork,
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
            if let Some(object) = self.state.objects.values().nth(cursor - 4)
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
                    && self.target_legal(object.id)
                {
                    candidates.push(visible.handle.clone());
                }
                if let Some(PlayerRequest::EntityTargets { candidates, .. }) = &mut b.decision
                    && self.target_legal(object.id)
                {
                    candidates.push(PlayerTargetEntity::Object {
                        object: visible.handle.clone(),
                    });
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
            let offset = 4
                + self.state.objects.len()
                + self.state.stack.len()
                + self.live_effect_keys().len();
            if cursor >= offset
                && let Some(id) = self.trigger_order_ids().get(cursor - offset)
            {
                candidates.push(
                    self.visible_pending_trigger(viewer, *id)
                        .map_err(|_| QueryRejection::Invariant)?,
                );
            }
        }
        b.cursor = b.cursor.checked_add(1).ok_or(QueryRejection::Invariant)?;
        let base = 4 + self.state.objects.len() + self.state.stack.len();
        let effect_count = self.live_effect_keys().len();
        if cursor >= base && cursor < base + effect_count {
            b.view.effects.push(
                self.visible_effect(viewer, &self.live_effect_keys()[cursor - base])
                    .map_err(|_| QueryRejection::Invariant)?,
            );
        }
        if let Some(PlayerRequest::DamageEffect { candidates, .. }) = &mut b.decision
            && cursor >= base + effect_count
            && let Some((_, _, pairs)) = self
                .damage_effect_group()
                .map_err(|_| QueryRejection::Invariant)?
            && let Some(pair) = pairs.get(cursor - base - effect_count)
        {
            candidates.push(
                self.visible_damage_effect_candidate(viewer, pair)
                    .map_err(|_| QueryRejection::Invariant)?,
            );
        }
        if let Some(PlayerRequest::Modes { candidates, .. }) = &mut b.decision
            && cursor >= base + effect_count
            && let Some(Request::Modes {
                candidates: ids, ..
            }) = self
                .modes_request()
                .map_err(|_| QueryRejection::Invariant)?
            && let Some(id) = ids.get(cursor - base - effect_count)
        {
            candidates.push(*id);
        }
        if let Some(PlayerRequest::DamageAllocation { candidates, .. }) = &mut b.decision
            && cursor >= base + effect_count
            && let Some(Request::DamageAllocation {
                candidates: ids, ..
            }) = self
                .damage_allocation_request()
                .map_err(|_| QueryRejection::Invariant)?
            && let Some(c) = ids.get(cursor - base - effect_count)
        {
            candidates.push(
                self.visible_allocation_candidate(viewer, c)
                    .map_err(|_| QueryRejection::Invariant)?,
            );
        }
        let extra = effect_count
            + if matches!(b.decision, Some(PlayerRequest::DamageAllocation { .. })) {
                self.damage_allocation_request()
                    .map_err(|_| QueryRejection::Invariant)?
                    .map_or(0, |r| {
                        if let Request::DamageAllocation { candidates, .. } = r {
                            candidates.len()
                        } else {
                            0
                        }
                    })
            } else if matches!(b.decision, Some(PlayerRequest::Modes { .. })) {
                self.modes_request()
                    .map_err(|_| QueryRejection::Invariant)?
                    .map_or(0, |r| {
                        if let Request::Modes { candidates, .. } = r {
                            candidates.len()
                        } else {
                            0
                        }
                    })
            } else if matches!(b.decision, Some(PlayerRequest::DamageEffect { .. })) {
                self.damage_effect_group()
                    .map_err(|_| QueryRejection::Invariant)?
                    .map_or(0, |(_, _, p)| p.len())
            } else if matches!(b.decision, Some(PlayerRequest::OrderTriggers { .. })) {
                self.trigger_order_ids().len()
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
    fn reference_query_unit(
        &self,
        data: &mut WorkData,
    ) -> Result<Option<QueryValue>, QueryRejection> {
        let invalid = |_: String| QueryRejection::InvalidResponse;
        match (&data.query, &mut data.task) {
            (Query::Observe { .. }, Task::Observe { builder }) => Ok(self
                .reference_observation_unit(builder)?
                .map(QueryValue::Observation)),
            (Query::Decision { viewer }, Task::Decision { cursor, request }) => {
                if let Some(PlayerRequest::OrderTriggers { candidates, .. }) = request {
                    let ids = self.trigger_order_ids();
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
                    let Some(Request::DamageAllocation {
                        candidates: ids, ..
                    }) = self
                        .damage_allocation_request()
                        .map_err(|_| QueryRejection::Invariant)?
                    else {
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
                    let Some(Request::Modes {
                        candidates: ids, ..
                    }) = self
                        .modes_request()
                        .map_err(|_| QueryRejection::Invariant)?
                    else {
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
                    let pairs = self
                        .damage_effect_group()
                        .map_err(|_| QueryRejection::Invariant)?
                        .map_or(vec![], |(_, _, p)| p);
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
                        self.state
                            .objects
                            .values()
                            .nth(cursor_index - 4)
                            .filter(|o| self.target_legal(o.id))
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
                    if let Some(o) = self.state.objects.values().nth(*cursor as usize)
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
                        (Frame::Modes { .. }, PlayerResponse::Modes { .. }) => true,
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
                            PlayerResponse::Sacrifice { .. }
                            | PlayerResponse::ResolveDiscard { .. },
                        )
                        | (Frame::DamageEffects { .. }, PlayerResponse::DamageEffect { .. })
                        | (Frame::SpendMana { .. }, PlayerResponse::SpendMana { .. })
                        | (Frame::CommanderReturn { .. }, PlayerResponse::CommanderReturn { .. })
                        | (Frame::Attackers, PlayerResponse::Attackers { .. })
                        | (Frame::Blockers { .. }, PlayerResponse::Blockers { .. })
                        | (Frame::DamageChoice { .. }, PlayerResponse::Damage { .. })
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
                    if !self.target_legal(id) {
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
                    if !self.entity_legal(target) {
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
                    *observation = self.reference_observation_unit(builder)?.map(Box::new);
                    return Ok(None);
                }
                let permitted = &self.state.visibility[viewer.0 as usize].history;
                if history.len() < permitted.len() {
                    history.push(permitted[history.len()].to_public());
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
}

fn query_value(value: QueryValue) -> serde_json::Value {
    match value {
        QueryValue::Observation(v) => serde_json::json!({"observation":v}),
        QueryValue::Decision(v) => serde_json::json!({"decision":v}),
        QueryValue::SearchContext(v) => serde_json::json!({"context":v}),
        QueryValue::Validated(v) => serde_json::json!({"input":v.input,"revision":v.revision}),
    }
}
fn reference_resume(
    game: &Game,
    data: &mut WorkData,
    mut budget: Budget,
) -> Result<Option<QueryValue>, QueryRejection> {
    while budget.consume() {
        data.units_done = data
            .units_done
            .checked_add(1)
            .ok_or(QueryRejection::Invariant)?;
        if let Some(value) = game.reference_query_unit(data)? {
            return Ok(Some(value));
        }
    }
    Ok(None)
}
fn compare(game: &Game, query: Query) {
    let before = game.state.clone();
    let trace = game.trace.clone();
    let state_sha256 = game.state_digest().unwrap();
    for quantum in [1, 64, u64::MAX] {
        let mut work = game.start_work(query.clone()).unwrap();
        let mut reference = work.data.clone();
        assert!(work.cache.effects.is_none());
        assert!(work.cache.last_object.is_none());
        // Budget0 leaves pure accelerators completely lazy and wire unchanged.
        let QueryResult::Incomplete {
            work: zero,
            required_minimum,
        } = game.resume_work(work, Budget::new(0))
        else {
            panic!("zero query budget")
        };
        assert_eq!(required_minimum, 1);
        work = zero;
        assert!(work.cache.effects.is_none());
        assert!(work.cache.last_object.is_none());
        let mut checkpoints = 0;
        loop {
            let expected = reference_resume(game, &mut reference, Budget::new(quantum));
            let actual = game.resume_work(work, Budget::new(quantum));
            match (actual, expected) {
                (QueryResult::Complete(a), Ok(Some(b))) => {
                    assert_eq!(query_value(a), query_value(b));
                    break;
                }
                (QueryResult::Rejected(a), Err(b)) => {
                    assert_eq!(a, b);
                    break;
                }
                (
                    QueryResult::Incomplete {
                        work: next,
                        required_minimum,
                    },
                    Ok(None),
                ) => {
                    assert_eq!(required_minimum, 1);
                    assert_eq!(next.data, reference);
                    work = next;
                    // Early and mid-object/effect/request prefixes exercise cold
                    // recovery. Comparing every unit's scratch above catches
                    // any change to the deterministic budget partition.
                    if (quantum == 1 && [1, 5, 31, 129].contains(&reference.units_done))
                        || (quantum == 64 && checkpoints == 0)
                    {
                        let saved = game.save_work(&work).unwrap();
                        assert_eq!(
                            saved,
                            canonical(&SavedWork {
                                work_version: 15,
                                build_id: BUILD_ID.into(),
                                catalog_digest: game.prepared.catalog_digest.clone(),
                                state_sha256: state_sha256.clone(),
                                data: reference.clone()
                            })
                            .unwrap()
                        );
                        let cold = game.restore_work(&saved).unwrap();
                        assert_eq!(cold.data, reference);
                        work = cold;
                        checkpoints += 1;
                    }
                }
                _ => panic!("optimized query changed reference completion/rejection unit"),
            }
        }
    }
    assert_eq!(game.state, before);
    assert_eq!(game.trace, trace);
    assert_eq!(game.state_digest().unwrap(), state_sha256);
}
pub(in crate::engine) fn compare_queries(game: &Game) {
    for viewer in (0..4).map(PlayerId) {
        compare(game, Query::Observe { viewer });
        compare(game, Query::Decision { viewer });
        if matches!(game.state.frame, Frame::Priority) && game.is_live(viewer) {
            compare(
                game,
                Query::Search {
                    viewer,
                    sampling_seed: "reference-policy-seed".into(),
                },
            );
        }
    }
    for player in game.live_players() {
        if let Some(request) = game.decision_for(player).unwrap() {
            let response = match request {
                Request::Priority { .. } => PlayerResponse::Pass,
                Request::Modes { .. } => PlayerResponse::Modes { mode: 0 },
                Request::Targets { .. } => PlayerResponse::Targets { objects: vec![] },
                Request::EntityTargets { .. } => PlayerResponse::EntityTargets { entities: vec![] },
                Request::OrderTriggers { .. } => PlayerResponse::OrderTriggers { order: vec![] },
                Request::DamageEffect { .. } => {
                    let QueryResult::Complete(QueryValue::Decision(Some(
                        PlayerRequest::DamageEffect { candidates, .. },
                    ))) = game.player_decision_for(player, Budget::unlimited())
                    else {
                        panic!("effect choice")
                    };
                    PlayerResponse::DamageEffect {
                        effect: candidates[0].effect.clone(),
                        damage: candidates[0].damage.clone(),
                    }
                }
                Request::DamageAllocation { .. } => PlayerResponse::DamageAllocation {
                    allocations: vec![],
                },
                _ => continue,
            };
            let token = game
                .request_header(player)
                .unwrap()
                .unwrap()
                .token()
                .clone();
            compare(
                game,
                Query::Check {
                    viewer: player,
                    token: token.clone(),
                    response,
                },
            );
            // Also compare successful multi-unit conversion prefixes, not only
            // the empty invalid vectors above. These remain part of SavedWork.
            let QueryResult::Complete(QueryValue::Decision(Some(public))) =
                game.player_decision_for(player, Budget::unlimited())
            else {
                panic!("public choice")
            };
            let valid = match public {
                PlayerRequest::Targets {
                    candidates,
                    occurrences,
                    ..
                } => candidates.first().map(|first| PlayerResponse::Targets {
                    objects: vec![first.clone(); occurrences.len()],
                }),
                PlayerRequest::EntityTargets {
                    candidates,
                    occurrences,
                    ..
                } => candidates
                    .first()
                    .map(|first| PlayerResponse::EntityTargets {
                        entities: vec![first.clone(); occurrences.len()],
                    }),
                PlayerRequest::OrderTriggers { candidates, .. } => {
                    Some(PlayerResponse::OrderTriggers {
                        order: candidates
                            .into_iter()
                            .map(|candidate| candidate.handle)
                            .collect(),
                    })
                }
                PlayerRequest::DamageAllocation {
                    candidates,
                    required,
                    ..
                } => {
                    let mut left = required;
                    Some(PlayerResponse::DamageAllocation {
                        allocations: candidates
                            .into_iter()
                            .map(|candidate| {
                                let amount = left.clone().min(candidate.amount);
                                left.subtract(&amount);
                                (candidate.damage, amount)
                            })
                            .collect(),
                    })
                }
                _ => None,
            };
            if let Some(response) = valid {
                compare(
                    game,
                    Query::Check {
                        viewer: player,
                        token,
                        response,
                    },
                );
            }
        }
    }
}
fn match_root() -> Game {
    let c = Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-match-ir.json"
    ))
    .unwrap();
    let mut game = Game::with_trace(
        Prepared::admit_pod(c, MatchConfig::foundations([0; 32], PlayerId(0))).unwrap(),
        TraceMode::Off,
    );
    for _ in 0..100 {
        let Advance::AwaitingDecision(request) = game.advance(Budget::unlimited()) else {
            panic!("opening")
        };
        if matches!(
            request,
            Request::Priority {
                player: PlayerId(0),
                ..
            }
        ) && game.turn().step == TurnStep::PrecombatMain
        {
            return game;
        }
        let response = match request {
            Request::Mulligan { .. } => Response::Mulligan { take: false },
            Request::Priority { .. } => Response::Pass,
            _ => panic!("unexpected opening choice"),
        };
        game.respond(request.player(), request.decision_id(), response)
            .unwrap();
    }
    panic!("opening did not finish")
}
fn fixture(catalog: &str, config: ScenarioConfig) -> Game {
    Game::with_trace(
        Prepared::admit(Catalog::read(catalog).unwrap(), config).unwrap(),
        TraceMode::Off,
    )
}
fn answer(game: &mut Game, response: Response) {
    let Advance::AwaitingDecision(request) = game.advance(Budget::unlimited()) else {
        panic!("fixture decision")
    };
    game.respond(request.player(), request.decision_id(), response)
        .unwrap();
}
#[test]
fn optimized_work_matches_original_every_unit_for_four_three_two_seat_roots() {
    let mut game = match_root();
    compare_queries(&game);
    for departing in [PlayerId(1), PlayerId(3)] {
        game.concede(departing).unwrap();
        let _ = game.advance(Budget::unlimited());
        // Removing the interior hundred-card owner proves range traversal keeps
        // ordinal cursors exact across large gaps in the physical-key map.
        compare_queries(&game);
    }
}
#[test]
fn optimized_work_matches_effects_modes_targets_and_trigger_candidate_domains() {
    let effects = include_str!("../../../../tests/fixtures/expected-effects-ir.json");
    compare_queries(&fixture(effects, ScenarioConfig::effects_two_furnaces()));
    let choices = include_str!("../../../../tests/fixtures/expected-choices-ir.json");
    let mut mode = fixture(choices, ScenarioConfig::choices());
    answer(&mut mode, Response::Cast { card: CardId(5) });
    let _ = mode.advance(Budget::unlimited());
    assert!(matches!(mode.state.frame, Frame::Modes { .. }));
    compare_queries(&mode);
    answer(&mut mode, Response::Modes { mode: 1 });
    let _ = mode.advance(Budget::unlimited());
    assert!(matches!(mode.state.frame, Frame::EntityTargets { .. }));
    compare_queries(&mode);
    let mut targets = fixture(choices, ScenarioConfig::choices());
    answer(&mut targets, Response::Cast { card: CardId(4) });
    let _ = targets.advance(Budget::unlimited());
    assert!(matches!(targets.state.frame, Frame::Targets { .. }));
    compare_queries(&targets);
    let mut triggers = fixture(
        include_str!("../../../../tests/fixtures/expected-trigger-ir.json"),
        ScenarioConfig::triggers(),
    );
    let source = triggers.object_id(CardId(5)).unwrap();
    answer(
        &mut triggers,
        Response::ActivateAbility {
            object: source,
            ability: 0,
        },
    );
    let _ = triggers.advance(Budget::unlimited());
    assert!(matches!(triggers.state.frame, Frame::TriggerOrder { .. }));
    compare_queries(&triggers);
}
fn shield(game: &mut Game, source: u32, amount: i64) {
    let source = game.object_id(CardId(source)).unwrap();
    let id = EffectId(game.state.next_effect_id);
    game.state.next_effect_id += 1;
    game.state.next_stack_id = 1;
    let source_handles = game.capture_trigger_source_handles(source).unwrap();
    game.state.effects.push(ResolvedEffect {
        id,
        target_handles: vec![],
        origin: EffectOrigin {
            mode: Some(1),
            stack_id: StackId(0),
            source,
            controller: PlayerId(0),
            source_handles,
            cursor: 0,
            source_block: 0,
        },
        expires_in_cleanup_of_turn: game.state.turn.number,
        kind: ResolvedEffectKind::PreventNextDamage {
            target: TargetEntity::Player {
                player: PlayerId(0),
            },
            remaining: amount.into(),
        },
    });
}
fn damage_sources(game: &Game) -> Vec<DamageOccurrence> {
    game.state
        .objects
        .values()
        .filter(|o| {
            o.zone == Zone::Battlefield
                && game.definition(o.id.card).unwrap().characteristics.kind == CardKind::Creature
        })
        .take(2)
        .map(|o| DamageOccurrence {
            source: o.id,
            controller: o.controller,
            recipient: DamageRecipient::Player {
                player: PlayerId(0),
            },
            amount: 3.into(),
        })
        .collect()
}
#[test]
fn optimized_work_matches_simultaneous_damage_effect_and_allocation_rows() {
    // Private event-domain fixtures isolate query enumeration; natural whole
    // shield/damage rules are covered by the independent effects/choice suites.
    let mut effects = fixture(
        include_str!("../../../../tests/fixtures/expected-effects-ir.json"),
        ScenarioConfig::effects_two_furnaces(),
    );
    let healer = effects
        .state
        .cards
        .iter()
        .find(|(_, c)| c.oracle_id == HEALER)
        .unwrap()
        .0
        .0;
    shield(&mut effects, healer, 1);
    let damage = damage_sources(&effects);
    effects.prepare_combat_damage(damage).unwrap();
    effects.refresh_visibility().unwrap();
    assert!(matches!(effects.state.frame, Frame::DamageEffects { .. }));
    compare_queries(&effects);
    let mut allocation = fixture(
        include_str!("../../../../tests/fixtures/expected-choices-ir.json"),
        ScenarioConfig::choices(),
    );
    shield(&mut allocation, 5, 3);
    let damage = damage_sources(&allocation);
    allocation.prepare_combat_damage(damage).unwrap();
    allocation.damage_effect_step().unwrap();
    allocation.refresh_visibility().unwrap();
    assert!(matches!(
        allocation.state.frame,
        Frame::DamageAllocation { .. }
    ));
    compare_queries(&allocation);
}
#[test]
fn optimized_work_caches_rehydrate_and_never_cross_executor_or_state_boundaries() {
    let mut game = match_root();
    let viewer = PlayerId(0);
    let QueryResult::Incomplete { work, .. } = game.observe(viewer, Budget::new(64)) else {
        panic!("mid observation")
    };
    assert!(work.cache.effects.is_some());
    assert!(work.cache.last_object.is_some());
    let saved = game.save_work(&work).unwrap();
    let fork = game.fork();
    assert!(matches!(
        fork.resume_work(work, Budget::unlimited()),
        QueryResult::Rejected(QueryRejection::StaleOrForeignWork)
    ));
    let recovered = fork.restore_work(&saved).unwrap();
    assert!(recovered.cache.effects.is_some());
    assert!(recovered.cache.last_object.is_some());
    let QueryResult::Complete(QueryValue::Observation(view)) =
        fork.resume_work(recovered, Budget::unlimited())
    else {
        panic!("recovered observation")
    };
    let QueryResult::Complete(QueryValue::Observation(expected)) =
        game.observe(viewer, Budget::unlimited())
    else {
        panic!("expected observation")
    };
    assert_eq!(view, expected);
    let mut tampered: serde_json::Value = serde_json::from_str(&saved).unwrap();
    tampered["data"]["task"]["builder"]["cursor"] = 1.into();
    assert!(game.restore_work(&tampered.to_string()).is_err());
    let QueryResult::Incomplete { work, .. } = game.observe(viewer, Budget::new(64)) else {
        panic!("new work")
    };
    game.concede(PlayerId(3)).unwrap();
    assert!(matches!(
        game.resume_work(work, Budget::unlimited()),
        QueryResult::Rejected(QueryRejection::StaleOrForeignWork)
    ));
    assert!(game.restore_work(&saved).is_err());
}
