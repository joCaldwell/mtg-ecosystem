//! Pure initiation discovery. Candidate construction is syntax, not legality:
//! every emitted beginning passes the ordinary budgeted player validator.
use super::*;

/// Native output bound, not a host payload/quota or elapsed-time guarantee.
pub const ACTION_PAGE_MAX: u16 = 64;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "scope", rename_all = "snake_case", deny_unknown_fields)]
pub enum ActionScope {
    Window,
    Object { object: ObjectHandle },
}

/// Finite beginnings; deliberately excludes target/payment/X combinations.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum ActionInitiation {
    Pass,
    PlayLand { object: ObjectHandle },
    Cast { card: ObjectHandle },
    ActivateMana { object: ObjectHandle },
    ActivateAbility { object: ObjectHandle, ability: u32 },
    FinishMana { undo_mana: bool },
}
impl ActionInitiation {
    pub fn response(&self) -> PlayerResponse {
        match self {
            Self::Pass => PlayerResponse::Pass,
            Self::PlayLand { object } => PlayerResponse::PlayLand {
                object: object.clone(),
            },
            Self::Cast { card } => PlayerResponse::Cast { card: card.clone() },
            Self::ActivateMana { object } => PlayerResponse::ActivateMana {
                object: object.clone(),
            },
            Self::ActivateAbility { object, ability } => PlayerResponse::ActivateAbility {
                object: object.clone(),
                ability: *ability,
            },
            Self::FinishMana { undo_mana } => PlayerResponse::FinishMana {
                undo_mana: *undo_mana,
            },
        }
    }
}

/// No compiler-owned semantic-to-source label mapping exists in this build.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ActionLabel {
    Unavailable,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ActionBeginning {
    pub initiation: ActionInitiation,
    pub label: ActionLabel,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum ActionSourceContext {
    Available { object: ObjectHandle },
    Unavailable,
}
/// Minimal context copied from the permitted request, with no inferred labels.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "window", rename_all = "snake_case", deny_unknown_fields)]
pub enum ActionDecisionContext {
    Priority,
    Mana {
        cost: ManaCost,
        source: ActionSourceContext,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ActionPage {
    pub token: DecisionToken,
    pub scope: ActionScope,
    /// None only while initialization has not received a budget unit.
    pub context: Option<ActionDecisionContext>,
    /// Incremental page; resume never repeats earlier emitted entries.
    pub beginnings: Vec<ActionBeginning>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ActionUnavailable {
    InvalidViewer,
    InvalidPageSize,
    Boundary,
    UnsupportedWindow,
    Scope,
    Invariant,
}
/// This is the permitted result. It contains no continuation, receipt or epoch.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum ActionQueryOutcome {
    Complete {
        page: ActionPage,
    },
    Incomplete {
        page: ActionPage,
        required_minimum: u64,
    },
    Stale,
    Unavailable {
        reason: ActionUnavailable,
    },
}

/// Host-only wrapper. Only `outcome` is public player data. A transport owner
/// must store the continuation privately and issue its own authenticated cursor.
/// No persistence/recovery serialization is supported; restart a query instead.
pub struct ActionQueryResult {
    outcome: ActionQueryOutcome,
    continuation: Option<ActionContinuation>,
}
impl ActionQueryResult {
    pub fn outcome(&self) -> &ActionQueryOutcome {
        &self.outcome
    }
    pub fn into_parts(self) -> (ActionQueryOutcome, Option<ActionContinuation>) {
        (self.outcome, self.continuation)
    }
    fn terminal(outcome: ActionQueryOutcome) -> Self {
        Self {
            outcome,
            continuation: None,
        }
    }
    fn unavailable(reason: ActionUnavailable) -> Self {
        Self::terminal(ActionQueryOutcome::Unavailable { reason })
    }
}

/// Private, non-serializable, single-owner query progress. Dropping/cancelling
/// it cannot change Game, accepted inputs, handles, history or any clock.
///
/// ```compile_fail
/// fn cannot_serialize(work: &mtg_core::ActionContinuation) {
///     serde_json::to_string(work).unwrap();
/// }
/// ```
pub struct ActionContinuation {
    executor: Arc<()>,
    state_token: Arc<()>,
    viewer: PlayerId,
    token: DecisionToken,
    scope: ActionScope,
    page_size: u16,
    context: Option<ActionDecisionContext>,
    global_index: u8,
    last_object: Option<ObjectHandle>,
    object: Option<ObjectCandidates>,
    checking: Option<(ActionInitiation, Work)>,
}
impl ActionContinuation {
    pub fn cancel(self) {}
}
struct ObjectCandidates {
    handle: ObjectHandle,
    index: usize,
    activated: usize,
}

impl Game {
    pub(super) fn begin_actions(
        &self,
        viewer: PlayerId,
        token: DecisionToken,
        scope: ActionScope,
        page_size: u16,
        budget: Budget,
    ) -> ActionQueryResult {
        if !(1..=ACTION_PAGE_MAX).contains(&page_size) {
            return ActionQueryResult::unavailable(ActionUnavailable::InvalidPageSize);
        }
        // Reuse existing query boundary guards without doing query work.
        if let QueryResult::Rejected(reason) =
            self.check_player_response(viewer, token.clone(), PlayerResponse::Pass, Budget::new(0))
        {
            return action_rejection(reason);
        }
        self.resume_actions(
            viewer,
            ActionContinuation {
                executor: self.executor.clone(),
                state_token: self.state_token.clone(),
                viewer,
                token,
                scope,
                page_size,
                context: None,
                global_index: 0,
                last_object: None,
                object: None,
                checking: None,
            },
            budget,
        )
    }

    pub(super) fn resume_actions(
        &self,
        viewer: PlayerId,
        mut work: ActionContinuation,
        mut budget: Budget,
    ) -> ActionQueryResult {
        if viewer != work.viewer
            || !Arc::ptr_eq(&work.executor, &self.executor)
            || !Arc::ptr_eq(&work.state_token, &self.state_token)
        {
            return ActionQueryResult::terminal(ActionQueryOutcome::Stale);
        }
        let mut beginnings = vec![];
        while budget.consume() {
            if work.context.is_none() {
                let request = match self.request_header(viewer) {
                    Ok(Some(request)) if request.token() == &work.token => request,
                    Ok(_) => return ActionQueryResult::terminal(ActionQueryOutcome::Stale),
                    Err(_) => return ActionQueryResult::unavailable(ActionUnavailable::Invariant),
                };
                work.context = Some(match request {
                    PlayerRequest::Priority { .. } => ActionDecisionContext::Priority,
                    PlayerRequest::Mana {
                        cost,
                        mana_ability_source,
                        ..
                    } => ActionDecisionContext::Mana {
                        cost,
                        source: mana_ability_source
                            .map_or(ActionSourceContext::Unavailable, |object| {
                                ActionSourceContext::Available { object }
                            }),
                    },
                    _ => {
                        return ActionQueryResult::unavailable(
                            ActionUnavailable::UnsupportedWindow,
                        );
                    }
                });
                if let ActionScope::Object { object } = &work.scope
                    && self.resolve_handle(viewer, object).is_err()
                {
                    return ActionQueryResult::unavailable(ActionUnavailable::Scope);
                }
                continue;
            }
            if let Some((initiation, pending)) = work.checking.take() {
                match self.resume_work(pending, Budget::new(1)) {
                    QueryResult::Complete(QueryValue::Validated(_)) => {
                        beginnings.push(ActionBeginning {
                            initiation,
                            label: ActionLabel::Unavailable,
                        });
                    }
                    QueryResult::Incomplete { work: pending, .. } => {
                        work.checking = Some((initiation, pending));
                    }
                    QueryResult::Rejected(QueryRejection::InvalidResponse) => (),
                    QueryResult::Rejected(reason) => return action_rejection(reason),
                    _ => return ActionQueryResult::unavailable(ActionUnavailable::Invariant),
                }
                if beginnings.len() == usize::from(work.page_size) {
                    break;
                }
                continue;
            }
            match self.next_action_candidate(&mut work) {
                Ok(Some(initiation)) => {
                    match self.check_player_response(
                        viewer,
                        work.token.clone(),
                        initiation.response(),
                        Budget::new(0),
                    ) {
                        QueryResult::Incomplete { work: pending, .. } => {
                            work.checking = Some((initiation, pending))
                        }
                        QueryResult::Rejected(reason) => return action_rejection(reason),
                        _ => return ActionQueryResult::unavailable(ActionUnavailable::Invariant),
                    }
                }
                Ok(None) => {
                    return ActionQueryResult::terminal(ActionQueryOutcome::Complete {
                        page: work.page(beginnings),
                    });
                }
                Err(reason) => return ActionQueryResult::unavailable(reason),
            }
        }
        ActionQueryResult {
            outcome: ActionQueryOutcome::Incomplete {
                page: work.page(beginnings),
                required_minimum: 1,
            },
            continuation: Some(work),
        }
    }

    fn next_action_candidate(
        &self,
        work: &mut ActionContinuation,
    ) -> Result<Option<ActionInitiation>, ActionUnavailable> {
        let priority = matches!(work.context, Some(ActionDecisionContext::Priority));
        if matches!(work.scope, ActionScope::Window) {
            let global = match (priority, work.global_index) {
                (true, 0) => Some(ActionInitiation::Pass),
                (false, 0) => Some(ActionInitiation::FinishMana { undo_mana: false }),
                (false, 1) => Some(ActionInitiation::FinishMana { undo_mana: true }),
                _ => None,
            };
            if global.is_some() {
                work.global_index += 1;
                return Ok(global);
            }
        }
        // Advance at most one permitted object here. Exhausted objects are
        // retired when producing their last syntactic candidate, never scanned
        // in an unbudgeted loop. No hidden row contributes an iteration.
        if work.object.is_none() {
            let handle = match &work.scope {
                ActionScope::Window => {
                    self.next_action_handle(work.viewer, work.last_object.as_ref())
                }
                ActionScope::Object { object } => {
                    work.last_object.is_none().then(|| object.clone())
                }
            };
            let Some(handle) = handle else {
                return Ok(None);
            };
            let id = self
                .resolve_handle(work.viewer, &handle)
                .map_err(|_| ActionUnavailable::Invariant)?;
            let activated = if priority {
                self.definition(id.card)
                    .map_err(|_| ActionUnavailable::Invariant)?
                    .activated
                    .len()
            } else {
                0
            };
            work.last_object = Some(handle.clone());
            work.object = Some(ObjectCandidates {
                handle,
                index: 0,
                activated,
            });
        }
        let object = work.object.as_mut().ok_or(ActionUnavailable::Invariant)?;
        let handle = object.handle.clone();
        let initiation = if priority {
            match object.index {
                0 => ActionInitiation::PlayLand { object: handle },
                1 => ActionInitiation::Cast { card: handle },
                2 => ActionInitiation::ActivateMana { object: handle },
                index => ActionInitiation::ActivateAbility {
                    object: handle,
                    ability: u32::try_from(index - 3).map_err(|_| ActionUnavailable::Invariant)?,
                },
            }
        } else {
            ActionInitiation::ActivateMana { object: handle }
        };
        object.index += 1;
        if object.index == if priority { 3 + object.activated } else { 1 } {
            work.object = None;
        }
        Ok(Some(initiation))
    }
}
impl ActionContinuation {
    fn page(&self, beginnings: Vec<ActionBeginning>) -> ActionPage {
        ActionPage {
            token: self.token.clone(),
            scope: self.scope.clone(),
            context: self.context.clone(),
            beginnings,
        }
    }
}
fn action_rejection(reason: QueryRejection) -> ActionQueryResult {
    match reason {
        QueryRejection::StaleOrForeignWork => {
            ActionQueryResult::terminal(ActionQueryOutcome::Stale)
        }
        QueryRejection::InvalidViewer => {
            ActionQueryResult::unavailable(ActionUnavailable::InvalidViewer)
        }
        QueryRejection::UnavailableDuringSetup | QueryRejection::UnsupportedBoundary => {
            ActionQueryResult::unavailable(ActionUnavailable::Boundary)
        }
        _ => ActionQueryResult::unavailable(ActionUnavailable::Invariant),
    }
}

#[cfg(test)]
#[path = "action_query_tests.rs"]
mod tests;
