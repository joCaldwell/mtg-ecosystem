//! Independent durable-ledger tests. Transition records/publication fixtures are
//! trusted producer data; these tests do not claim Game validation or HTTP auth.
use mtg_core::{DecisionToken, Observation, PlayerResponse};
use mtg_server::{
    protocol::{
        CommandBody, CommandEnvelope, Counter, Publication, RejectionReason, SubmissionResult,
    },
    store::{CommandCommit, CommitResult, Head, MatchStore, Transition},
};
use rusqlite::Connection;
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "mtg-store-adversarial-{}-{}",
            std::process::id(),
            NEXT_DIRECTORY.fetch_add(1, Ordering::SeqCst)
        ));
        assert!(!path.exists());
        Self(path)
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
fn hash(n: u64) -> String {
    format!("{n:064x}")
}
fn bootstrap() -> Value {
    json!({"build":"pinned-fixture","private_seed":[1,2,3],"schema":1})
}
fn open(d: &Directory) -> MatchStore {
    MatchStore::open(&d.0, &bootstrap(), &hash(0)).unwrap()
}
fn request(id: u64, seat: u8) -> CommandEnvelope {
    CommandEnvelope {
        protocol_version: 1,
        command_id: format!("{id:032x}"),
        expected_revision: Counter::new(0),
        command: CommandBody::Decision {
            token: DecisionToken(format!("seat-{seat}-token")),
            response: PlayerResponse::Pass,
        },
    }
}
fn accepted(id: u64, seat: u8) -> CommandCommit {
    let request = request(id, seat);
    CommandCommit {
        seat,
        result: SubmissionResult::Accepted {
            command_id: request.command_id.clone(),
        },
        request,
    }
}
fn transition(expected: Head, id: u64, command: Option<CommandCommit>) -> Transition {
    Transition {
        next_boundary: expected.boundary + 1,
        state_sha256: hash(id),
        record: json!({"fixture_input":id}),
        expected,
        command,
        execution_updates: vec![],
        publications: std::array::from_fn(|_| vec![]),
    }
}
fn commit(store: &mut MatchStore, t: Transition) -> Head {
    match store.commit(store.generation(), t).unwrap() {
        CommitResult::Committed(h) => h,
        CommitResult::ExistingSubmission(_) => panic!("unexpected duplicate"),
    }
}
fn publication(seat: u8) -> Publication {
    let observation: Observation = serde_json::from_value(json!({
        "observation_version":15,
        "view":{"combat":[],"viewer":seat,"active":0,"turn_owner":0,
            "turn":{"number":1,"step":"precombat_main","lands_played":0},
            "live_turn_order":[0,1,2,3],"commanders":[],"priority":0,
            "players":[],"stack":[],"objects":[{
                "marked_damage":"0","attack_ready":false,"handle":format!("seat-{seat}-private"),
                "oracle_id":format!("00000000-0000-0000-0000-{seat:012}"),
                "owner":seat,"controller":seat,"zone":{"zone":"hand","owner":seat},"tapped":false
            }]},"decision":null
    }))
    .unwrap();
    Publication::ViewReplaced {
        observation: Box::new(observation),
    }
}
fn assert_empty(s: &MatchStore, expected: &Head) {
    assert_eq!(&s.head().unwrap(), expected);
    assert!(s.records_after(0, 128).unwrap().is_empty());
    for seat in 0..4 {
        assert!(s.stream_page(seat, 0, 32).unwrap().entries.is_empty());
    }
}

#[test]
fn duplicate_command_precedes_stale_head_and_conflicts_never_reapply() {
    let d = Directory::new();
    let mut s = open(&d);
    let initial = s.head().unwrap();
    let first = commit(&mut s, transition(initial.clone(), 1, Some(accepted(1, 0))));
    let last = commit(&mut s, transition(first.clone(), 2, None));
    let duplicate = s
        .commit(
            s.generation(),
            transition(initial.clone(), 1, Some(accepted(1, 0))),
        )
        .unwrap();
    assert_eq!(
        duplicate,
        CommitResult::ExistingSubmission(SubmissionResult::Accepted {
            command_id: request(1, 0).command_id
        })
    );
    assert_eq!(s.head().unwrap(), last);
    assert_eq!(s.records_after(0, 128).unwrap().len(), 2);
    let mut conflict = accepted(1, 0);
    conflict.request.expected_revision = Counter::new(9);
    assert!(
        s.commit(s.generation(), transition(initial, 3, Some(conflict)))
            .unwrap_err()
            .contains("different command")
    );
    assert_eq!(s.head().unwrap(), last);
    // Command IDs are principal-scoped, rather than globally unique across seats.
    let head = commit(&mut s, transition(last, 3, Some(accepted(1, 1))));
    assert_eq!(head.sequence, 3);
    assert!(s.existing_submission(0, &request(1, 0)).unwrap().is_some());
    assert!(s.existing_submission(1, &request(1, 1)).unwrap().is_some());
}

#[test]
fn late_sqlite_failure_rolls_back_records_outbox_command_and_head_together() {
    let d = Directory::new();
    let mut s = open(&d);
    let initial = s.head().unwrap();
    d.sql().execute_batch("CREATE TRIGGER injected_failure BEFORE UPDATE ON head BEGIN SELECT RAISE(ABORT,'injected late failure'); END;").unwrap();
    let mut t = transition(initial.clone(), 1, Some(accepted(1, 0)));
    t.publications[0].push(publication(0));
    assert!(
        s.commit(s.generation(), t)
            .unwrap_err()
            .contains("injected late failure")
    );
    assert_empty(&s, &initial);
    assert!(s.existing_submission(0, &request(1, 0)).unwrap().is_none());
    d.sql()
        .execute_batch("DROP TRIGGER injected_failure;")
        .unwrap();
    let mut t = transition(initial, 1, Some(accepted(1, 0)));
    t.publications[0].push(publication(0));
    assert_eq!(commit(&mut s, t).sequence, 1);
    assert_eq!(s.stream_page(0, 0, 32).unwrap().entries.len(), 1);
}

#[test]
fn late_cursor_overflow_or_invalid_publication_preserves_the_old_durable_state() {
    let d = Directory::new();
    let mut s = open(&d);
    let initial = s.head().unwrap();
    d.sql()
        .execute("UPDATE viewer_heads SET cursor=?1 WHERE seat=3", [i64::MAX])
        .unwrap();
    let mut t = transition(initial.clone(), 1, Some(accepted(1, 0)));
    t.publications[0].push(publication(0));
    t.publications[3].push(publication(3));
    assert!(
        s.commit(s.generation(), t)
            .unwrap_err()
            .contains("cursor exhausted")
    );
    assert_eq!(s.head().unwrap(), initial);
    assert!(s.records_after(0, 128).unwrap().is_empty());
    assert!(s.stream_page(0, 0, 32).unwrap().entries.is_empty());
    assert!(s.existing_submission(0, &request(1, 0)).unwrap().is_none());
    let saved: i64 = d
        .sql()
        .query_row("SELECT cursor FROM viewer_heads WHERE seat=3", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(saved, i64::MAX);
    d.sql()
        .execute("UPDATE viewer_heads SET cursor=0 WHERE seat=3", [])
        .unwrap();
    let mut t = transition(initial.clone(), 1, Some(accepted(1, 0)));
    t.publications[0].push(publication(0));
    let mut unsafe_view = publication(3);
    let Publication::ViewReplaced { observation } = &mut unsafe_view else {
        unreachable!()
    };
    observation.view.turn.number = 1_u64 << 53;
    t.publications[3].push(unsafe_view);
    assert!(s.commit(s.generation(), t).is_err());
    assert_empty(&s, &initial);
    assert!(s.existing_submission(0, &request(1, 0)).unwrap().is_none());
}

#[test]
fn reopen_retains_exact_input_chain_rejections_idempotency_and_per_seat_streams() {
    let d = Directory::new();
    let mut s = open(&d);
    let generation = s.generation();
    let bootstrap_hash = s.bootstrap_sha256().unwrap();
    let mut head = s.head().unwrap();
    for id in 1..=4 {
        let mut command = accepted(id, (id % 4) as u8);
        if id == 3 {
            command.result = SubmissionResult::Rejected {
                command_id: command.request.command_id.clone(),
                reason: RejectionReason::InvalidResponse,
            };
        }
        let mut t = transition(head, id, Some(command));
        t.publications[(id % 4) as usize].push(publication((id % 4) as u8));
        head = commit(&mut s, t);
    }
    let records = s.records_after(0, 128).unwrap();
    let pages = (0..4)
        .map(|p| s.stream_page(p, 0, 32).unwrap())
        .collect::<Vec<_>>();
    drop(s);
    let s = open(&d);
    assert_eq!(s.generation(), generation + 1);
    assert_eq!(s.head().unwrap(), head);
    assert_eq!(s.bootstrap_sha256().unwrap(), bootstrap_hash);
    assert_eq!(s.records_after(0, 128).unwrap(), records);
    for (seat, page) in pages.into_iter().enumerate() {
        assert_eq!(s.stream_page(seat as u8, 0, 32).unwrap(), page);
    }
    let rejected = s.existing_submission(3, &request(3, 3)).unwrap().unwrap();
    assert!(matches!(
        rejected,
        SubmissionResult::Rejected {
            reason: RejectionReason::InvalidResponse,
            ..
        }
    ));
    for pair in records.windows(2) {
        assert_eq!(pair[1].head.sequence, pair[0].head.sequence + 1);
        assert_eq!(pair[1].before_boundary, pair[0].head.boundary);
        assert_eq!(pair[1].before_state_sha256, pair[0].head.state_sha256);
    }
}

#[test]
fn exclusive_lock_bootstrap_pinning_and_persisted_generation_fence_are_enforced() {
    let d = Directory::new();
    let mut s = open(&d);
    let first = s.generation();
    assert!(MatchStore::open(&d.0, &bootstrap(), &hash(0)).is_err());
    let initial = s.head().unwrap();
    assert!(
        s.commit(first + 1, transition(initial.clone(), 1, None))
            .is_err()
    );
    assert_empty(&s, &initial);
    d.sql()
        .execute(
            "UPDATE metadata SET value=?1 WHERE key='generation'",
            [(first + 1).to_string()],
        )
        .unwrap();
    assert!(
        s.commit(first, transition(initial.clone(), 1, None))
            .is_err()
    );
    assert_empty(&s, &initial);
    drop(s);
    assert!(MatchStore::open(&d.0, &json!({"build":"different"}), &hash(0)).is_err());
    assert!(MatchStore::open(&d.0, &bootstrap(), &hash(9)).is_err());
    let s = open(&d);
    assert_eq!(s.generation(), first + 2);
    assert_eq!(s.head().unwrap(), initial);
}

#[test]
fn private_outbox_pagination_and_cursors_do_not_cross_seats() {
    let d = Directory::new();
    let mut s = open(&d);
    let initial = s.head().unwrap();
    let mut t = transition(initial, 1, None);
    t.publications[0].push(publication(0));
    t.publications[2] = vec![publication(2), publication(2)];
    let h = commit(&mut s, t);
    let mut t = transition(h, 2, None);
    t.publications[0] = vec![publication(0), publication(0)];
    t.publications[1].push(publication(1));
    t.publications[3].push(publication(3));
    commit(&mut s, t);
    for seat in 0..4 {
        let mut after = 0;
        let mut count = 0;
        loop {
            let page = s.stream_page(seat, after, 1).unwrap();
            assert!(page.entries.len() <= 1);
            for entry in &page.entries {
                assert_eq!(entry.cursor.get(), after + 1);
                let Publication::ViewReplaced { observation } = &entry.publication else {
                    panic!("expected view")
                };
                assert_eq!(observation.view.viewer.0, seat);
                assert_eq!(
                    observation.view.objects[0].handle.0,
                    format!("seat-{seat}-private")
                );
                count += 1;
            }
            after = page.next.get();
            if !page.has_more {
                break;
            }
        }
        assert_eq!(count, [3, 1, 2, 1][seat as usize]);
        assert!(s.stream_page(seat, after + 1, 1).is_err());
    }
    for (seat, limit) in [(4, 1), (0, 0), (0, 33)] {
        assert!(s.stream_page(seat, 0, limit).is_err());
    }
    for limit in [0, 129] {
        assert!(s.records_after(0, limit).is_err());
    }
}

#[test]
fn boundary_digest_record_and_publication_quotas_fail_without_partial_commit() {
    let d = Directory::new();
    let mut s = open(&d);
    let initial = s.head().unwrap();
    for bad in ["", "ABCDEF", &"A".repeat(64), &"0".repeat(63)] {
        let mut t = transition(initial.clone(), 1, None);
        t.state_sha256 = bad.into();
        assert!(s.commit(s.generation(), t).is_err());
    }
    let mut big = transition(initial.clone(), 1, None);
    big.record = json!("x".repeat(131_072));
    assert!(s.commit(s.generation(), big).is_err());
    let mut many = transition(initial.clone(), 1, None);
    many.publications[3] = vec![publication(3); 3];
    assert!(s.commit(s.generation(), many).is_err());
    let mut large = publication(3);
    let Publication::ViewReplaced { observation } = &mut large else {
        unreachable!()
    };
    observation.view.objects[0].handle.0 = "x".repeat(65_536);
    let mut t = transition(initial.clone(), 1, Some(accepted(1, 0)));
    t.publications[0].push(publication(0));
    t.publications[3].push(large);
    assert!(s.commit(s.generation(), t).is_err());
    assert_empty(&s, &initial);
    let h = commit(&mut s, transition(initial, 1, None));
    let mut backward = transition(h.clone(), 2, None);
    backward.next_boundary = h.boundary - 1;
    assert!(s.commit(s.generation(), backward).is_err());
    assert_eq!(s.head().unwrap(), h);
    // Accepted inputs may change state without executing another rule step.
    let mut same_boundary = transition(h.clone(), 2, None);
    same_boundary.next_boundary = h.boundary;
    assert_eq!(commit(&mut s, same_boundary).boundary, h.boundary);
}

#[test]
fn a_projection_for_another_viewer_cannot_enter_the_wrong_private_outbox() {
    let d = Directory::new();
    let mut s = open(&d);
    let initial = s.head().unwrap();
    let mut t = transition(initial.clone(), 1, Some(accepted(1, 0)));
    t.publications[0].push(publication(0));
    t.publications[3].push(publication(1));
    assert!(
        s.commit(s.generation(), t).is_err(),
        "wrong viewer accepted into seat3 outbox"
    );
    assert_empty(&s, &initial);
    assert!(s.existing_submission(0, &request(1, 0)).unwrap().is_none());
}

#[cfg(unix)]
#[test]
fn store_requires_a_preexisting_parent_and_never_initializes_a_nonprivate_directory() {
    use std::os::unix::fs::DirBuilderExt;
    let missing = Directory::new();
    let nested = missing.0.join("unsynced-ancestor").join("match");
    assert!(MatchStore::open(&nested, &bootstrap(), &hash(0)).is_err());
    assert!(
        !missing.0.exists(),
        "store silently created unsynced ancestors"
    );
    let public = Directory::new();
    std::fs::DirBuilder::new()
        .mode(0o755)
        .create(&public.0)
        .unwrap();
    assert!(MatchStore::open(&public.0, &bootstrap(), &hash(0)).is_err());
    assert!(!public.0.join("match.sqlite3").exists());
    assert!(!public.0.join("owner.lock").exists());
}

#[test]
fn recovery_record_byte_bound_applies_before_text_materialization() {
    use mtg_contracts::canonical;
    use mtg_server::store::MAX_RECORD_BYTES;
    let directory = Directory::new();
    let mut store = open(&directory);
    let before = store.head().unwrap();
    commit(&mut store, transition(before, 1, None));
    let mut record = store.records_after(0, 1).unwrap().remove(0);
    record.record = json!({"padding":""});
    let overhead = canonical(&record).unwrap().len();
    record.record["padding"] = "x".repeat(MAX_RECORD_BYTES - overhead).into();
    let exact = canonical(&record).unwrap();
    assert_eq!(exact.len(), MAX_RECORD_BYTES);
    let sql = directory.sql();
    sql.execute("UPDATE records SET body=?1", [&exact]).unwrap();
    assert_eq!(store.records_after(0, 1).unwrap(), vec![record.clone()]);
    sql.execute("UPDATE records SET body=?1", [format!("{exact} ")])
        .unwrap();
    assert!(store.records_after(0, 1).is_err());
    // Text code points and text-before-NUL lengths are not byte bounds.
    for padding in [
        "é".repeat(MAX_RECORD_BYTES / 2),
        format!("\\u0000{}", "x".repeat(MAX_RECORD_BYTES)),
    ] {
        record.record["padding"] = padding.into();
        let oversized = canonical(&record).unwrap();
        assert!(oversized.len() > MAX_RECORD_BYTES);
        sql.execute("UPDATE records SET body=?1", [&oversized])
            .unwrap();
        assert!(store.records_after(0, 1).is_err());
    }
    // A NUL suffix may hide a large tail from SQLite length(TEXT).
    sql.execute(
        "UPDATE records SET body=?1",
        [format!("{exact}\0{}", "x".repeat(MAX_RECORD_BYTES))],
    )
    .unwrap();
    assert!(store.records_after(0, 1).is_err());
    sql.execute("UPDATE records SET body=?1", [exact.as_bytes()])
        .unwrap();
    assert!(store.records_after(0, 1).is_err());
    sql.execute("UPDATE records SET body=?1", [&exact]).unwrap();
    assert!(store.records_after(0, 0).is_err());
    assert!(store.records_after(0, 129).is_err());
    assert!(store.records_after(1, 6).unwrap().is_empty());
}
