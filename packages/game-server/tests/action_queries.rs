//! Bounded worker transport invariants over frozen admitted local catalogs.
use mtg_core::{ActionScope, DecisionToken, MatchConfig, PlayerId, PlayerRequest, PlayerResponse};
use mtg_server::{
    protocol::*,
    worker::{Worker, WorkerPhase},
};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        Self(std::env::temp_dir().join(format!(
            "mtg-actions-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )))
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if self.0.exists() {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }
}
fn settle(w: &mut Worker) {
    for _ in 0..2000 {
        if !matches!(w.host_state().phase, WorkerPhase::Running) {
            return;
        }
        w.advance(Instant::now()).unwrap();
    }
    panic!("setup limit")
}
fn offered(w: &Worker) -> (u8, PlayerRequest) {
    for seat in 0..4 {
        if let ResumeState::Available { observation } = w.resume(seat).unwrap().state
            && let Some(request) = observation.decision
        {
            return (seat, request);
        }
    }
    panic!("missing offered request")
}
fn prefix() -> (Directory, Worker, u8, DecisionToken) {
    let dir = Directory::new();
    let mut w = Worker::create(
        &dir.0,
        mtg_contracts::Catalog::read(include_str!(
            "../../game-engine/tests/fixtures/expected-match-ir.json"
        ))
        .unwrap(),
        MatchConfig::for_profile("commander-foundations-v1", [3; 32], PlayerId(0)).unwrap(),
        "a1",
        Instant::now(),
    )
    .unwrap();
    settle(&mut w);
    for n in 0..4 {
        let (seat, r) = offered(&w);
        assert!(matches!(r, PlayerRequest::Mulligan { .. }));
        let revision = w.host_state().viewer_cursors[usize::from(seat)];
        assert!(matches!(
            w.submit(
                seat,
                CommandEnvelope {
                    protocol_version: 1,
                    command_id: format!("{n:032x}"),
                    expected_revision: Counter::new(revision),
                    command: CommandBody::Decision {
                        token: r.token().clone(),
                        response: PlayerResponse::Mulligan { take: false }
                    }
                },
                Instant::now()
            )
            .unwrap(),
            SubmissionResult::Accepted { .. }
        ));
        settle(&mut w);
    }
    let (seat, r) = offered(&w);
    assert!(matches!(r, PlayerRequest::Priority { .. }));
    (dir, w, seat, r.token().clone())
}
fn start(token: DecisionToken) -> ActionQueryRequest {
    ActionQueryRequest::Start {
        token,
        scope: ActionScope::Window,
        page_size: 1,
        work: 1,
    }
}
fn cursor(response: ActionQueryResponse) -> String {
    if let ActionQueryStatus::Incomplete { cursor, .. } = response.outcome {
        cursor
    } else {
        panic!("expected incomplete: {response:?}")
    }
}
#[test]
fn action_query_pages_are_pure_single_use_viewer_bound_cancelled_and_expiring() {
    let (_dir, mut w, seat, token) = prefix();
    let now = Instant::now();
    let state = w.state_sha256().unwrap();
    let host = w.host_state().clone();
    let head = w.head().clone();
    let views: Vec<_> = (0..4).map(|s| w.resume(s).unwrap()).collect();
    let c = cursor(w.query_actions(seat, start(token.clone()), now));
    assert!(matches!(
        w.query_actions(
            (seat + 1) % 4,
            ActionQueryRequest::Continue {
                cursor: c.clone(),
                work: 1
            },
            now
        )
        .outcome,
        ActionQueryStatus::Stale
    ));
    let c2 = cursor(w.query_actions(
        seat,
        ActionQueryRequest::Continue {
            cursor: c.clone(),
            work: 1,
        },
        now,
    ));
    assert_ne!(c, c2);
    assert!(matches!(
        w.query_actions(
            seat,
            ActionQueryRequest::Continue { cursor: c, work: 1 },
            now
        )
        .outcome,
        ActionQueryStatus::Stale
    ));
    assert!(matches!(
        w.query_actions(seat, ActionQueryRequest::Cancel { cursor: c2.clone() }, now)
            .outcome,
        ActionQueryStatus::Cancelled
    ));
    assert!(matches!(
        w.query_actions(seat, ActionQueryRequest::Cancel { cursor: c2 }, now)
            .outcome,
        ActionQueryStatus::Stale
    ));
    let c = cursor(w.query_actions(seat, start(token), now));
    assert!(matches!(
        w.query_actions(
            seat,
            ActionQueryRequest::Continue { cursor: c, work: 1 },
            now + Duration::from_secs(30)
        )
        .outcome,
        ActionQueryStatus::Stale
    ));
    assert_eq!(w.state_sha256().unwrap(), state);
    assert_eq!(*w.host_state(), host);
    assert_eq!(*w.head(), head);
    for s in 0..4 {
        assert_eq!(w.resume(s).unwrap(), views[usize::from(s)]);
    }
}
#[test]
fn action_query_quota_total_work_and_recovery_do_not_change_rules() {
    let (dir, mut w, seat, token) = prefix();
    let now = Instant::now();
    let state = w.state_sha256().unwrap();
    let c = cursor(w.query_actions(seat, start(token.clone()), now));
    for _ in 1..16 {
        assert!(matches!(
            w.query_actions(seat, start(token.clone()), now).outcome,
            ActionQueryStatus::Unavailable {
                reason: ActionQueryUnavailable::InFlight
            }
        ));
    }
    assert!(matches!(
        w.query_actions(seat, start(token.clone()), now).outcome,
        ActionQueryStatus::Unavailable {
            reason: ActionQueryUnavailable::Quota
        }
    ));
    // Cancellation remains available under quota.
    assert!(matches!(
        w.query_actions(seat, ActionQueryRequest::Cancel { cursor: c }, now)
            .outcome,
        ActionQueryStatus::Cancelled
    ));
    let c = cursor(w.query_actions(seat, start(token), now + Duration::from_secs(1)));
    assert_eq!(w.state_sha256().unwrap(), state);
    drop(w);
    let mut recovered = Worker::recover(
        &dir.0,
        mtg_contracts::Catalog::read(include_str!(
            "../../game-engine/tests/fixtures/expected-match-ir.json"
        ))
        .unwrap(),
        "a1",
        Instant::now(),
    )
    .unwrap();
    assert!(matches!(
        recovered
            .query_actions(
                seat,
                ActionQueryRequest::Continue { cursor: c, work: 1 },
                Instant::now()
            )
            .outcome,
        ActionQueryStatus::Stale
    ));
}
#[test]
fn action_query_completion_and_stale_tokens_preserve_core_context() {
    let (_dir, mut w, seat, token) = prefix();
    let now = Instant::now();
    let result = w.query_actions(
        seat,
        ActionQueryRequest::Start {
            token: token.clone(),
            scope: ActionScope::Window,
            page_size: 32,
            work: 1000,
        },
        now,
    );
    assert!(matches!(result.outcome, ActionQueryStatus::Complete { .. }));
    let value = serde_json::to_value(&result).unwrap();
    let schema = serde_json::to_value(action_query_schema()).unwrap();
    jsonschema::draft7::options()
        .build(&schema)
        .unwrap()
        .validate(&value)
        .unwrap();
    let text = checked_actions(&result).unwrap();
    assert!(!text.contains("continuation"));
    assert!(!text.contains("receipt"));
    assert!(!text.contains("epoch"));
    let foreign: DecisionToken = serde_json::from_str("\"v999-d999\"").unwrap();
    assert!(matches!(
        w.query_actions(seat, start(foreign), now).outcome,
        ActionQueryStatus::Stale
    ));
    // A committed command replaces executor state and clears every continuation.
    let c = cursor(w.query_actions(seat, start(token.clone()), now));
    let revision = w.host_state().viewer_cursors[usize::from(seat)];
    w.submit(
        seat,
        CommandEnvelope {
            protocol_version: 1,
            command_id: "f".repeat(32),
            expected_revision: Counter::new(revision),
            command: CommandBody::Decision {
                token,
                response: PlayerResponse::Pass,
            },
        },
        Instant::now(),
    )
    .unwrap();
    settle(&mut w);
    assert!(matches!(
        w.query_actions(
            seat,
            ActionQueryRequest::Continue { cursor: c, work: 1 },
            now
        )
        .outcome,
        ActionQueryStatus::Stale
    ));
}
#[test]
fn action_request_strict_decode_and_schema_bounds() {
    let valid = r#"{"operation":"start","token":"v0-d0","scope":{"scope":"window"},"page_size":32,"work":1000}"#;
    decode_action_request(valid).unwrap();
    let validator = jsonschema::draft7::options()
        .build(&serde_json::to_value(action_request_schema()).unwrap())
        .unwrap();
    assert!(validator.is_valid(&serde_json::from_str::<serde_json::Value>(valid).unwrap()));
    for bad in [
        valid.replace("1000", "1001"),
        valid.replace("1000", "0"),
        valid.replace("32", "33"),
        valid.replace("\"work\":1000", "\"work\":1,\"work\":1"),
        valid.replace("\"work\":1000", "\"work\":1,\"viewer\":1"),
        valid.replace("\"scope\":\"window\"", "\"scope\":\"window\",\"seed\":0"),
    ] {
        assert!(decode_action_request(&bad).is_err(), "{bad}");
    }
}
