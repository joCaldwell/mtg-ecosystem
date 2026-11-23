//! Independent transactional execution-disposition tests. Rules interpretation
//! remains the worker's responsibility; these records are trusted producer data.
use mtg_server::{
    protocol::{
        CommandBody, CommandEnvelope, CommandExecutionStatus as Status, Counter, ExecutionStatus,
        Publication, RejectionReason, SubmissionResult,
    },
    store::{CommandCommit, CommitResult, ExecutionUpdate, Head, MatchStore, Transition},
};
use rusqlite::Connection;
use serde_json::json;
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "mtg-execution-adversarial-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        assert!(!path.exists());
        Self(path)
    }
    fn open(&self) -> MatchStore {
        MatchStore::open(
            &self.0,
            &json!({"trusted_execution_fixture":1}),
            &format!("{:064x}", 0),
        )
        .unwrap()
    }
    fn sql(&self) -> Connection {
        Connection::open(self.0.join("match.sqlite3")).unwrap()
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if self.0.exists() {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }
}
fn id(n: u64) -> String {
    format!("{n:032x}")
}
fn command(seat: u8, n: u64, reason: Option<RejectionReason>) -> CommandCommit {
    let command_id = id(n);
    CommandCommit {
        seat,
        request: CommandEnvelope {
            protocol_version: 1,
            command_id: command_id.clone(),
            expected_revision: Counter::new(0),
            command: CommandBody::Concede,
        },
        result: match reason {
            None => SubmissionResult::Accepted { command_id },
            Some(reason) => SubmissionResult::Rejected { command_id, reason },
        },
    }
}
fn transition(store: &MatchStore) -> Transition {
    let expected = store.head().unwrap();
    Transition {
        next_boundary: expected.boundary + 1,
        state_sha256: format!("{:064x}", expected.sequence + 1),
        record: json!({"trusted_transition":expected.sequence+1}),
        expected,
        command: None,
        publications: std::array::from_fn(|_| vec![]),
        execution_updates: vec![],
    }
}
fn commit(store: &mut MatchStore, transition: Transition) -> Head {
    match store.commit(store.generation(), transition).unwrap() {
        CommitResult::Committed(head) => head,
        CommitResult::ExistingSubmission(_) => panic!("unexpected duplicate fixture command"),
    }
}
fn insert(store: &mut MatchStore, seat: u8, n: u64, reason: Option<RejectionReason>) {
    let mut t = transition(store);
    t.command = Some(command(seat, n, reason));
    commit(store, t);
}
fn update(seat: u8, n: u64, status: Status) -> ExecutionUpdate {
    ExecutionUpdate {
        seat,
        command_id: id(n),
        status,
    }
}
fn status(store: &MatchStore, seat: u8, n: u64) -> Option<Status> {
    store.execution(seat, &id(n)).unwrap().map(|r| {
        assert_eq!(r.command_id, id(n));
        r.status
    })
}
fn assert_unchanged(store: &MatchStore, head: &Head, records: usize, cursors: [u64; 4]) {
    assert_eq!(&store.head().unwrap(), head);
    assert_eq!(store.records_after(0, 128).unwrap().len(), records);
    for seat in 0..4 {
        assert_eq!(
            store.viewer_cursor(seat).unwrap(),
            cursors[usize::from(seat)]
        );
    }
}
fn published_transition(store: &MatchStore) -> Transition {
    let mut t = transition(store);
    t.publications[0].push(Publication::Lifecycle {
        status: ExecutionStatus::Waiting,
    });
    t
}

#[test]
fn initial_disposition_is_derived_from_immutable_submission_and_scoped_to_seat() {
    let d = Directory::new();
    let mut s = d.open();
    insert(&mut s, 0, 1, None);
    insert(&mut s, 1, 1, Some(RejectionReason::InvalidResponse));
    insert(&mut s, 2, 1, Some(RejectionReason::Cancelled));
    assert_eq!(status(&s, 0, 1), Some(Status::Pending));
    assert_eq!(
        status(&s, 1, 1),
        Some(Status::Rejected {
            reason: RejectionReason::InvalidResponse
        })
    );
    assert_eq!(status(&s, 2, 1), Some(Status::Cancelled));
    assert_eq!(status(&s, 3, 1), None);
    for seat in 0..4 {
        let expected = u64::from(seat != 3);
        assert_eq!(s.command_count(seat).unwrap(), expected);
        assert_eq!(s.execution_count(seat).unwrap(), expected);
    }
    assert!(s.execution(4, &id(1)).is_err());
    assert!(s.execution_count(4).is_err());
    let head = s.head().unwrap();
    drop(s);
    let s = d.open();
    assert_eq!(s.head().unwrap(), head);
    assert_eq!(status(&s, 0, 1), Some(Status::Pending));
    assert_eq!(
        status(&s, 1, 1),
        Some(Status::Rejected {
            reason: RejectionReason::InvalidResponse
        })
    );
    assert_eq!(status(&s, 2, 1), Some(Status::Cancelled));
}

#[test]
fn four_pending_dispositions_commit_with_record_and_outbox_then_reopen_exactly() {
    let d = Directory::new();
    let mut s = d.open();
    for seat in 0..4 {
        insert(&mut s, seat, 7, None);
    }
    let expected = [
        Status::ConsumedByRules,
        Status::DiscardedByDeparture,
        Status::ConsumedByRules,
        Status::DiscardedByDeparture,
    ];
    let mut t = published_transition(&s);
    t.execution_updates = expected
        .iter()
        .enumerate()
        .map(|(seat, status)| update(seat as u8, 7, status.clone()))
        .collect();
    let head = commit(&mut s, t);
    assert_eq!(head.sequence, 5);
    for seat in 0..4 {
        assert_eq!(
            status(&s, seat, 7),
            Some(expected[usize::from(seat)].clone())
        );
        let submission = s
            .existing_submission(seat, &command(seat, 7, None).request)
            .unwrap();
        assert_eq!(
            submission,
            Some(SubmissionResult::Accepted { command_id: id(7) })
        );
    }
    let records = s.records_after(0, 128).unwrap();
    let pages = std::array::from_fn::<_, 4, _>(|seat| s.stream_page(seat as u8, 0, 32).unwrap());
    assert_eq!(pages[0].entries.len(), 1);
    assert!(pages[1..].iter().all(|p| p.entries.is_empty()));
    drop(s);
    let s = d.open();
    assert_eq!(s.head().unwrap(), head);
    assert_eq!(s.records_after(0, 128).unwrap(), records);
    for seat in 0..4 {
        assert_eq!(
            status(&s, seat, 7),
            Some(expected[usize::from(seat)].clone())
        );
        assert_eq!(
            s.stream_page(seat, 0, 32).unwrap(),
            pages[usize::from(seat)]
        );
    }
}

#[test]
fn terminal_and_rejected_dispositions_cannot_be_rewritten_or_reopened() {
    let d = Directory::new();
    let mut s = d.open();
    insert(&mut s, 0, 1, None);
    insert(&mut s, 0, 2, None);
    insert(&mut s, 0, 3, Some(RejectionReason::Cancelled));
    insert(&mut s, 0, 4, Some(RejectionReason::StaleRevision));
    let mut t = transition(&s);
    t.execution_updates = vec![
        update(0, 1, Status::ConsumedByRules),
        update(0, 2, Status::DiscardedByDeparture),
    ];
    let head = commit(&mut s, t);
    for n in 1..=4 {
        let prior = status(&s, 0, n);
        for disposition in [
            Status::ConsumedByRules,
            Status::DiscardedByDeparture,
            Status::Pending,
            Status::Cancelled,
            Status::Rejected {
                reason: RejectionReason::InvalidResponse,
            },
        ] {
            let mut t = published_transition(&s);
            t.execution_updates = vec![update(0, n, disposition)];
            assert!(s.commit(s.generation(), t).is_err());
            assert_unchanged(&s, &head, 5, [0; 4]);
            assert_eq!(status(&s, 0, n), prior);
        }
    }
}

#[test]
fn invalid_late_updates_roll_back_earlier_dispositions_new_command_and_outbox() {
    let d = Directory::new();
    let mut s = d.open();
    insert(&mut s, 1, 1, None);
    insert(&mut s, 2, 2, None);
    let head = s.head().unwrap();
    let invalid_batches = vec![
        vec![
            update(1, 1, Status::ConsumedByRules),
            update(0, 1, Status::ConsumedByRules),
        ], // wrong seat
        vec![
            update(1, 1, Status::ConsumedByRules),
            update(2, 99, Status::ConsumedByRules),
        ], // orphan
        vec![
            update(1, 1, Status::ConsumedByRules),
            update(4, 2, Status::ConsumedByRules),
        ],
        vec![
            update(1, 1, Status::ConsumedByRules),
            update(1, 1, Status::DiscardedByDeparture),
        ],
        vec![
            update(1, 1, Status::ConsumedByRules),
            update(2, 2, Status::Pending),
        ],
        (0..5)
            .map(|n| update(0, n, Status::ConsumedByRules))
            .collect(),
    ];
    for updates in invalid_batches {
        let mut t = published_transition(&s);
        t.command = Some(command(3, 3, None));
        t.execution_updates = updates;
        assert!(s.commit(s.generation(), t).is_err());
        assert_unchanged(&s, &head, 2, [0; 4]);
        assert_eq!(status(&s, 1, 1), Some(Status::Pending));
        assert_eq!(status(&s, 2, 2), Some(Status::Pending));
        assert_eq!(status(&s, 3, 3), None);
        assert_eq!(s.command_count(3).unwrap(), 0);
        assert_eq!(s.execution_count(3).unwrap(), 0);
    }
}

#[test]
fn failure_at_final_head_write_rolls_back_execution_and_immutable_submission() {
    let d = Directory::new();
    let mut s = d.open();
    insert(&mut s, 0, 1, None);
    let head = s.head().unwrap();
    d.sql().execute_batch("CREATE TRIGGER injected_failure BEFORE UPDATE ON head BEGIN SELECT RAISE(ABORT,'late execution failure'); END;").unwrap();
    let make = |store: &MatchStore| {
        let mut t = published_transition(store);
        t.command = Some(command(1, 2, None));
        t.execution_updates = vec![update(0, 1, Status::ConsumedByRules)];
        t
    };
    let t = make(&s);
    assert!(
        s.commit(s.generation(), t)
            .unwrap_err()
            .contains("late execution failure")
    );
    assert_unchanged(&s, &head, 1, [0; 4]);
    assert_eq!(status(&s, 0, 1), Some(Status::Pending));
    assert_eq!(status(&s, 1, 2), None);
    d.sql()
        .execute_batch("DROP TRIGGER injected_failure;")
        .unwrap();
    let t = make(&s);
    commit(&mut s, t);
    assert_eq!(status(&s, 0, 1), Some(Status::ConsumedByRules));
    assert_eq!(status(&s, 1, 2), Some(Status::Pending));
    assert_eq!(s.viewer_cursor(0).unwrap(), 1);
}

#[test]
fn duplicate_submission_cannot_execute_again_or_rewrite_a_final_disposition() {
    let d = Directory::new();
    let mut s = d.open();
    insert(&mut s, 0, 1, None);
    let mut t = transition(&s);
    t.execution_updates = vec![update(0, 1, Status::ConsumedByRules)];
    commit(&mut s, t);
    let head = s.head().unwrap();
    let mut t = published_transition(&s);
    t.expected.sequence = 0;
    t.command = Some(command(0, 1, None));
    t.execution_updates = vec![update(0, 1, Status::DiscardedByDeparture)];
    assert_eq!(
        s.commit(s.generation(), t).unwrap(),
        CommitResult::ExistingSubmission(SubmissionResult::Accepted { command_id: id(1) })
    );
    assert_unchanged(&s, &head, 2, [0; 4]);
    assert_eq!(status(&s, 0, 1), Some(Status::ConsumedByRules));
}

#[test]
fn forged_pending_row_cannot_turn_an_immutable_rejection_into_rules_consumption() {
    let d = Directory::new();
    let mut s = d.open();
    for (n, reason) in [
        (1, RejectionReason::InvalidResponse),
        (2, RejectionReason::Cancelled),
    ] {
        insert(&mut s, 0, n, Some(reason));
        d.sql()
            .execute(
                "UPDATE execution SET status=?1 WHERE seat=0 AND command_id=?2",
                ["{\"status\":\"pending\"}".to_owned(), id(n)],
            )
            .unwrap();
        let head = s.head().unwrap();
        let records = s.records_after(0, 128).unwrap().len();
        let mut t = published_transition(&s);
        t.execution_updates = vec![update(0, n, Status::ConsumedByRules)];
        assert!(s.commit(s.generation(), t).is_err());
        assert_unchanged(&s, &head, records, [0; 4]);
        assert_eq!(status(&s, 0, n), Some(Status::Pending));
        assert_eq!(
            s.existing_submission(0, &command(0, n, Some(reason)).request)
                .unwrap(),
            Some(SubmissionResult::Rejected {
                command_id: id(n),
                reason
            })
        );
    }
}

#[test]
fn accepted_result_for_a_different_id_cannot_authorize_execution_update() {
    let d = Directory::new();
    let mut s = d.open();
    insert(&mut s, 0, 1, None);
    let head = s.head().unwrap();
    d.sql()
        .execute(
            "UPDATE commands SET result=?1 WHERE seat=0 AND command_id=?2",
            [
                serde_json::to_string(&SubmissionResult::Accepted { command_id: id(99) }).unwrap(),
                id(1),
            ],
        )
        .unwrap();
    let mut t = published_transition(&s);
    t.execution_updates = vec![update(0, 1, Status::ConsumedByRules)];
    assert!(s.commit(s.generation(), t).is_err());
    assert_unchanged(&s, &head, 1, [0; 4]);
    assert_eq!(status(&s, 0, 1), Some(Status::Pending));
}

#[test]
fn forged_out_of_domain_rows_in_all_four_tables_fail_load_before_generation_changes() {
    for table in ["commands", "execution", "viewer_heads", "outbox"] {
        for seat in ["4", "-1", "1.5", "'not-a-seat'"] {
            // INTEGER PRIMARY KEY rejects noninteger keys before the load check.
            if table == "viewer_heads" && !matches!(seat, "4" | "-1") {
                continue;
            }
            let d = Directory::new();
            drop(d.open());
            let sql = d.sql();
            let generation: String = sql
                .query_row(
                    "SELECT value FROM metadata WHERE key='generation'",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            // Test-only external corruption, including pre-CHECK legacy stores.
            sql.execute_batch("PRAGMA ignore_check_constraints=ON; PRAGMA foreign_keys=OFF;")
                .unwrap();
            let statement = match table {
                "commands" => format!("INSERT INTO commands VALUES({seat},'forged','{{}}','{{}}')"),
                "execution" => format!(
                    "INSERT INTO execution VALUES({seat},'forged','{{\"status\":\"pending\"}}')"
                ),
                "viewer_heads" => format!("INSERT INTO viewer_heads VALUES({seat},0)"),
                "outbox" => format!("INSERT INTO outbox VALUES({seat},1,'{{}}')"),
                _ => unreachable!(),
            };
            sql.execute_batch(&statement).unwrap();
            drop(sql);
            let failure = MatchStore::open(
                &d.0,
                &json!({"trusted_execution_fixture":1}),
                &format!("{:064x}", 0),
            )
            .err()
            .expect("corrupt seat-domain row must reject loading");
            assert!(
                failure.contains("invalid seat domain"),
                "{table}/{seat}: {failure}"
            );
            let current: String = d
                .sql()
                .query_row(
                    "SELECT value FROM metadata WHERE key='generation'",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(generation, current);
        }
    }
}
