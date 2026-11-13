//! Host-private scratch. Never serialized, persisted, or shared between workers.
use crate::protocol::{
    ActionQueryRequest, ActionQueryStatus, ActionQueryUnavailable, MAX_ACTION_RESPONSE_BYTES,
};
use mtg_core::{ActionContinuation, ActionQueryOutcome, Budget, Game, PlayerId};
use std::time::{Duration, Instant};

pub(crate) const TTL: Duration = Duration::from_secs(30);
const CALLS_PER_SECOND: u32 = 16;
const WORK_PER_SECOND: u32 = 8000;
const TOTAL_WORK: u32 = 32_000;
const TOTAL_CALLS: u32 = 128;
struct Pending {
    cursor: String,
    expires: Instant,
    work: u32,
    calls: u32,
    continuation: ActionContinuation,
}
#[derive(Default)]
struct Seat {
    pending: Option<Pending>,
    window: Option<Instant>,
    calls: u32,
    work: u32,
}
#[derive(Default)]
pub(crate) struct ActionQueries {
    seats: [Seat; 4],
}
fn unavailable(reason: ActionQueryUnavailable) -> ActionQueryStatus {
    ActionQueryStatus::Unavailable { reason }
}
impl ActionQueries {
    pub(crate) fn clear(&mut self) {
        // Keep quotas across state transitions; invalid commands cannot refill them.
        for seat in &mut self.seats {
            seat.pending = None;
        }
    }
    pub(crate) fn query(
        &mut self,
        game: &Game,
        seat: u8,
        request: ActionQueryRequest,
        now: Instant,
    ) -> ActionQueryStatus {
        // Reap all seats on every query, including cancellation. Idle abandoned
        // work is at most four continuations; expiry never extends on activity.
        for seat in &mut self.seats {
            if seat.pending.as_ref().is_some_and(|p| now >= p.expires) {
                seat.pending = None;
            }
        }
        let Some(owner) = self.seats.get_mut(usize::from(seat)) else {
            return unavailable(ActionQueryUnavailable::Executor);
        };
        if let ActionQueryRequest::Cancel { cursor } = &request {
            if owner.pending.as_ref().is_none_or(|p| p.cursor != *cursor) {
                return ActionQueryStatus::Stale;
            }
            owner.pending.take().unwrap().continuation.cancel();
            return ActionQueryStatus::Cancelled;
        }
        // Foreign/used/expired cursors always have the same response, regardless
        // of quotas or whether another viewer currently owns scratch work.
        if let ActionQueryRequest::Continue { cursor, .. } = &request
            && owner.pending.as_ref().is_none_or(|p| p.cursor != *cursor)
        {
            return ActionQueryStatus::Stale;
        }
        let work = match request {
            ActionQueryRequest::Start { work, .. } | ActionQueryRequest::Continue { work, .. } => {
                work
            }
            _ => unreachable!(),
        };
        if owner
            .window
            .is_none_or(|start| now.saturating_duration_since(start) >= Duration::from_secs(1))
        {
            owner.window = Some(now);
            owner.calls = 0;
            owner.work = 0;
        }
        if owner.calls >= CALLS_PER_SECOND || owner.work.saturating_add(work) > WORK_PER_SECOND {
            return unavailable(ActionQueryUnavailable::Quota);
        }
        owner.calls += 1;
        owner.work += work;
        let session = game.player_session(PlayerId(seat));
        let (result, expires, total_work, calls) = match request {
            ActionQueryRequest::Start {
                token,
                scope,
                page_size,
                ..
            } => {
                if owner.pending.is_some() {
                    return unavailable(ActionQueryUnavailable::InFlight);
                }
                (
                    session.action_beginnings(
                        token,
                        scope,
                        page_size,
                        Budget::new(u64::from(work)),
                    ),
                    now + TTL,
                    work,
                    1,
                )
            }
            ActionQueryRequest::Continue { .. } => {
                let pending = owner.pending.take().unwrap();
                let total = pending.work.saturating_add(work);
                if total > TOTAL_WORK || pending.calls >= TOTAL_CALLS {
                    return unavailable(ActionQueryUnavailable::TotalWork);
                }
                (
                    session.resume_actions(pending.continuation, Budget::new(u64::from(work))),
                    pending.expires,
                    total,
                    pending.calls + 1,
                )
            }
            _ => unreachable!(),
        };
        let (outcome, continuation) = result.into_parts();
        let status = match (outcome, continuation) {
            (ActionQueryOutcome::Complete { page }, None) => ActionQueryStatus::Complete { page },
            (
                ActionQueryOutcome::Incomplete {
                    page,
                    required_minimum,
                },
                Some(continuation),
            ) => {
                let mut bytes = [0; 32];
                if getrandom::fill(&mut bytes).is_err() {
                    return unavailable(ActionQueryUnavailable::Executor);
                }
                let cursor: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
                owner.pending = Some(Pending {
                    cursor: cursor.clone(),
                    expires,
                    work: total_work,
                    calls,
                    continuation,
                });
                ActionQueryStatus::Incomplete {
                    page,
                    required_minimum,
                    cursor,
                }
            }
            (ActionQueryOutcome::Stale, None) => ActionQueryStatus::Stale,
            (ActionQueryOutcome::Unavailable { reason }, None) => {
                unavailable(ActionQueryUnavailable::Core { reason })
            }
            _ => unavailable(ActionQueryUnavailable::Executor),
        };
        // Bound the scratch-to-wire conversion before IPC/HTTP, with room for
        // the identity envelope. Never truncate a page or publish a partial menu.
        match serde_json::to_vec(&status) {
            Ok(bytes) if bytes.len() <= MAX_ACTION_RESPONSE_BYTES - 1024 => status,
            _ => {
                owner.pending = None;
                unavailable(ActionQueryUnavailable::ResponseLimit)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mtg_core::{ActionScope, Prepared, QueryResult, QueryValue, ScenarioConfig};
    fn setup() -> (Game, ActionQueries, ActionQueryRequest) {
        let game = Game::new(
            Prepared::admit(
                mtg_contracts::Catalog::read(include_str!(
                    "../../game-engine/tests/fixtures/expected-ir.json"
                ))
                .unwrap(),
                ScenarioConfig::land_return(),
            )
            .unwrap(),
        );
        let QueryResult::Complete(QueryValue::Observation(o)) = game
            .player_session(PlayerId(0))
            .observe(Budget::unlimited())
        else {
            panic!("observation")
        };
        let request = ActionQueryRequest::Start {
            token: o.decision.unwrap().token().clone(),
            scope: ActionScope::Window,
            page_size: 1,
            work: 1,
        };
        (game, ActionQueries::default(), request)
    }
    #[test]
    fn total_allowance_and_call_ceiling_destroy_scratch_but_never_rules() {
        for calls in [false, true] {
            let (game, mut table, request) = setup();
            let now = Instant::now();
            let before = game.state_digest().unwrap();
            let ActionQueryStatus::Incomplete { cursor, .. } = table.query(&game, 0, request, now)
            else {
                panic!("incomplete")
            };
            let pending = table.seats[0].pending.as_mut().unwrap();
            if calls {
                pending.calls = TOTAL_CALLS
            } else {
                pending.work = TOTAL_WORK
            }
            assert_eq!(
                table.query(
                    &game,
                    0,
                    ActionQueryRequest::Continue {
                        cursor: cursor.clone(),
                        work: 1
                    },
                    now
                ),
                unavailable(ActionQueryUnavailable::TotalWork)
            );
            assert!(table.seats[0].pending.is_none());
            assert_eq!(game.state_digest().unwrap(), before);
            assert_eq!(
                table.query(
                    &game,
                    0,
                    ActionQueryRequest::Continue { cursor, work: 1 },
                    now
                ),
                ActionQueryStatus::Stale
            );
        }
    }
    #[test]
    fn work_rate_quota_does_not_consume_the_own_cursor_and_deadline_never_slides() {
        let (game, mut table, request) = setup();
        let now = Instant::now();
        let ActionQueryStatus::Incomplete { cursor, .. } = table.query(&game, 0, request, now)
        else {
            panic!("incomplete")
        };
        table.seats[0].work = WORK_PER_SECOND;
        assert_eq!(
            table.query(
                &game,
                0,
                ActionQueryRequest::Continue {
                    cursor: cursor.clone(),
                    work: 1
                },
                now
            ),
            unavailable(ActionQueryUnavailable::Quota)
        );
        assert_eq!(table.seats[0].pending.as_ref().unwrap().cursor, cursor);
        let ActionQueryStatus::Incomplete { cursor, .. } = table.query(
            &game,
            0,
            ActionQueryRequest::Continue { cursor, work: 1 },
            now + Duration::from_secs(1),
        ) else {
            panic!("resume")
        };
        assert_eq!(table.seats[0].pending.as_ref().unwrap().expires, now + TTL);
        assert_eq!(
            table.query(
                &game,
                0,
                ActionQueryRequest::Continue { cursor, work: 1 },
                now + TTL
            ),
            ActionQueryStatus::Stale
        );
        assert!(table.seats[0].pending.is_none());
    }
}
