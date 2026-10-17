//! Public Worker recovery must prove every prefix, including unchanged States,
//! private publications and an accepted input not yet consumed by the rules.
use mtg_contracts::{Catalog, canonical, digest};
use mtg_core::{MatchConfig, Observation, PlayerId, PlayerRequest, PlayerResponse};
use mtg_server::{
    protocol::{
        CommandBody, CommandEnvelope, CommandExecutionStatus, Counter, Publication, StreamEntry,
        SubmissionResult,
    },
    worker::{ClockControl, Worker, WorkerPhase},
};
use rusqlite::{Connection, types::ValueRef};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::Instant,
};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "mtg-recovery-public-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
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
fn catalog() -> Catalog {
    Catalog::read(include_str!(
        "../../game-engine/tests/fixtures/expected-match-ir.json"
    ))
    .unwrap()
}
fn settle(worker: &mut Worker) {
    for _ in 0..2_000 {
        match worker.host_state().phase {
            WorkerPhase::Running => {
                worker.advance(Instant::now()).unwrap();
            }
            WorkerPhase::Waiting => return,
            ref other => panic!("unexpected finite-prefix phase: {other:?}"),
        }
    }
    panic!("automatic setup bound")
}
fn stream(worker: &Worker, seat: u8) -> Vec<StreamEntry> {
    let mut entries = vec![];
    let mut cursor = 0;
    loop {
        let page = worker.stream_page(seat, cursor, 32).unwrap();
        cursor = page.next.get();
        entries.extend(page.entries);
        if !page.has_more {
            return entries;
        }
    }
}
fn offered(worker: &Worker) -> Observation {
    for seat in 0..4 {
        let latest = stream(worker, seat)
            .into_iter()
            .filter_map(|entry| match entry.publication {
                Publication::ViewReplaced { observation } => Some(*observation),
                _ => None,
            })
            .next_back();
        if let Some(observation) = latest
            && observation.decision.is_some()
        {
            return observation;
        }
    }
    panic!("no actual offered decision")
}
fn submit(worker: &mut Worker, ordinal: u64, response: PlayerResponse) -> (u8, String) {
    let observation = offered(worker);
    let seat = observation.view.viewer.0;
    let request = observation.decision.unwrap();
    let command_id = format!("{ordinal:032x}");
    let envelope = CommandEnvelope {
        protocol_version: 1,
        command_id: command_id.clone(),
        expected_revision: Counter::new(worker.host_state().viewer_cursors[usize::from(seat)]),
        command: CommandBody::Decision {
            token: request.token().clone(),
            response,
        },
    };
    assert_eq!(
        worker.submit(seat, envelope, Instant::now()).unwrap(),
        SubmissionResult::Accepted {
            command_id: command_id.clone()
        }
    );
    (seat, command_id)
}
fn prefix() -> (Directory, Worker, u8, String) {
    let directory = Directory::new();
    let mut worker = Worker::create(
        &directory.0,
        catalog(),
        MatchConfig::foundations([77; 32], PlayerId(1)),
        "a1",
        Instant::now(),
    )
    .unwrap();
    settle(&mut worker);
    for ordinal in 1..=4 {
        assert!(matches!(
            offered(&worker).decision,
            Some(PlayerRequest::Mulligan { .. })
        ));
        submit(
            &mut worker,
            ordinal,
            PlayerResponse::Mulligan { take: false },
        );
        settle(&mut worker);
    }
    assert!(matches!(
        offered(&worker).decision,
        Some(PlayerRequest::Priority { .. })
    ));
    let state = worker.state_sha256().unwrap();
    for _ in 0..7 {
        worker.clock(ClockControl::Pause, Instant::now()).unwrap();
        worker.clock(ClockControl::Resume, Instant::now()).unwrap();
        assert_eq!(worker.state_sha256().unwrap(), state);
    }
    while !worker.head().sequence.is_multiple_of(6) {
        worker.clock(ClockControl::Tick, Instant::now()).unwrap();
    }
    let (seat, id) = submit(&mut worker, 5, PlayerResponse::Pass);
    assert!(worker.head().sequence > 12);
    assert_eq!(worker.head().sequence % 6, 1, "partial final proof batch");
    assert_eq!(
        worker.command_execution(seat, &id).unwrap().unwrap().status,
        CommandExecutionStatus::Pending
    );
    assert_eq!(worker.host_state().outstanding.len(), 1);
    (directory, worker, seat, id)
}
fn fingerprint(directory: &Path) -> BTreeMap<String, String> {
    let connection = Connection::open(directory.join("match.sqlite3")).unwrap();
    let queries = [
        ("head", "SELECT * FROM head ORDER BY singleton"),
        ("records", "SELECT * FROM records ORDER BY sequence"),
        (
            "commands",
            "SELECT * FROM commands ORDER BY seat,command_id",
        ),
        (
            "execution",
            "SELECT * FROM execution ORDER BY seat,command_id",
        ),
        ("viewer_heads", "SELECT * FROM viewer_heads ORDER BY seat"),
        ("outbox", "SELECT * FROM outbox ORDER BY seat,cursor"),
        (
            "metadata",
            "SELECT * FROM metadata WHERE key!='generation' ORDER BY key",
        ),
    ];
    queries
        .into_iter()
        .map(|(name, sql)| {
            let mut statement = connection.prepare(sql).unwrap();
            let rows = statement
                .query_map([], |row| {
                    (0..row.as_ref().column_count())
                        .map(|index| {
                            Ok(match row.get_ref(index)? {
                                ValueRef::Null => Value::Null,
                                ValueRef::Integer(value) => value.into(),
                                ValueRef::Text(value) => std::str::from_utf8(value).unwrap().into(),
                                other => panic!("unexpected fixture SQL type: {other:?}"),
                            })
                        })
                        .collect::<rusqlite::Result<Vec<Value>>>()
                })
                .unwrap()
                .collect::<rusqlite::Result<Vec<_>>>()
                .unwrap();
            (name.into(), digest(&rows).unwrap())
        })
        .collect()
}
fn generation(directory: &Path) -> u64 {
    Connection::open(directory.join("match.sqlite3"))
        .unwrap()
        .query_row(
            "SELECT value FROM metadata WHERE key='generation'",
            [],
            |row| row.get::<_, String>(0),
        )
        .unwrap()
        .parse()
        .unwrap()
}

#[test]
fn full_and_partial_batches_restore_pending_input_and_exact_private_streams() {
    let (directory, worker, seat, id) = prefix();
    let head = worker.head().clone();
    let host = worker.host_state().clone();
    let identity = worker.identity().clone();
    let state = worker.state_sha256().unwrap();
    let streams: [_; 4] = std::array::from_fn(|seat| stream(&worker, seat as u8));
    drop(worker);
    let before = fingerprint(&directory.0);
    let previous_generation = generation(&directory.0);
    let mut recovered = Worker::recover(&directory.0, catalog(), "a1", Instant::now()).unwrap();
    assert_eq!(generation(&directory.0), previous_generation + 1);
    assert_eq!(recovered.head(), &head);
    assert_eq!(recovered.host_state(), &host);
    assert_eq!(recovered.identity(), &identity);
    assert_eq!(recovered.state_sha256().unwrap(), state);
    assert_eq!(fingerprint(&directory.0), before);
    for seat in 0..4 {
        assert_eq!(stream(&recovered, seat), streams[usize::from(seat)]);
    }
    assert_eq!(
        recovered
            .command_execution(seat, &id)
            .unwrap()
            .unwrap()
            .status,
        CommandExecutionStatus::Pending
    );
    settle(&mut recovered);
    assert!(recovered.host_state().outstanding.is_empty());
    assert_eq!(
        recovered
            .command_execution(seat, &id)
            .unwrap()
            .unwrap()
            .status,
        CommandExecutionStatus::ConsumedByRules
    );
    while !recovered.head().sequence.is_multiple_of(6) {
        recovered.clock(ClockControl::Tick, Instant::now()).unwrap();
    }
    let head = recovered.head().clone();
    let host = recovered.host_state().clone();
    let state = recovered.state_sha256().unwrap();
    drop(recovered);
    let before = fingerprint(&directory.0);
    let previous_generation = generation(&directory.0);
    let recovered = Worker::recover(&directory.0, catalog(), "a1", Instant::now()).unwrap();
    assert_eq!(generation(&directory.0), previous_generation + 1);
    assert_eq!(recovered.head(), &head);
    assert_eq!(recovered.host_state(), &host);
    assert_eq!(recovered.state_sha256().unwrap(), state);
    assert_eq!(fingerprint(&directory.0), before);
}

#[test]
fn internally_connected_middle_hash_forgery_rejects_with_valid_later_final_state() {
    let (directory, worker, _, _) = prefix();
    let final_head = worker.head().clone();
    drop(worker);
    let connection = directory.sql();
    let mut records = connection
        .prepare("SELECT body FROM records ORDER BY sequence")
        .unwrap()
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .map(|row| serde_json::from_str::<Value>(&row.unwrap()).unwrap())
        .collect::<Vec<_>>();
    let middle = (6..records.len() - 2)
        .find(|index| {
            records[*index]["record"]["action"]["action"] == "clock"
                && records[*index + 1]["record"]["action"]["action"] == "clock"
        })
        .expect("two unchanged-State clock records after a full batch");
    assert_eq!(
        records[middle]["head"]["state_sha256"],
        records[middle]["before_state_sha256"]
    );
    records[middle]["head"]["state_sha256"] = "0".repeat(64).into();
    records[middle + 1]["before_state_sha256"] = "0".repeat(64).into();
    for record in &records[middle..=middle + 1] {
        connection
            .execute(
                "UPDATE records SET body=?1 WHERE sequence=?2",
                rusqlite::params![
                    canonical(record).unwrap(),
                    i64::try_from(record["head"]["sequence"].as_u64().unwrap()).unwrap()
                ],
            )
            .unwrap();
    }
    assert_eq!(
        records.last().unwrap()["head"],
        serde_json::to_value(&final_head).unwrap()
    );
    drop(connection);
    let tampered = fingerprint(&directory.0);
    let error = Worker::recover(&directory.0, catalog(), "a1", Instant::now())
        .err()
        .expect("middle after-State proof is mandatory");
    assert!(error.contains("after-State hash mismatch"), "{error}");
    assert_eq!(fingerprint(&directory.0), tampered);
}

#[test]
fn late_private_stream_or_pending_execution_corruption_cannot_publish_a_worker() {
    for execution in [false, true] {
        let (directory, worker, seat, id) = prefix();
        let head = worker.head().clone();
        drop(worker);
        let connection = directory.sql();
        if execution {
            assert_eq!(
                connection
                    .execute(
                        "UPDATE execution SET status=?1 WHERE seat=?2 AND command_id=?3",
                        rusqlite::params![
                            canonical(&CommandExecutionStatus::ConsumedByRules).unwrap(),
                            seat,
                            id
                        ],
                    )
                    .unwrap(),
                1
            );
        } else {
            let rows = connection
                .prepare("SELECT seat,cursor,body FROM outbox ORDER BY cursor DESC,seat")
                .unwrap()
                .query_map([], |row| {
                    Ok((
                        row.get::<_, u8>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                })
                .unwrap()
                .collect::<rusqlite::Result<Vec<_>>>()
                .unwrap();
            let (seat, cursor, mut page) = rows
                .into_iter()
                .map(|(seat, cursor, text)| {
                    (seat, cursor, serde_json::from_str::<Value>(&text).unwrap())
                })
                .find(|(_, _, page)| page["entries"][0]["publication"]["kind"] == "view_replaced")
                .unwrap();
            let turn =
                &mut page["entries"][0]["publication"]["observation"]["view"]["turn"]["number"];
            *turn = (turn.as_u64().unwrap() + 1).into();
            assert_eq!(
                connection
                    .execute(
                        "UPDATE outbox SET body=?1 WHERE seat=?2 AND cursor=?3",
                        rusqlite::params![canonical(&page).unwrap(), seat, cursor]
                    )
                    .unwrap(),
                1
            );
        }
        assert_eq!(
            connection
                .query_row("SELECT state_sha FROM head", [], |row| row
                    .get::<_, String>(0))
                .unwrap(),
            head.state_sha256
        );
        drop(connection);
        let tampered = fingerprint(&directory.0);
        let error = Worker::recover(&directory.0, catalog(), "a1", Instant::now())
            .err()
            .expect("valid final State hash does not replace private ledger proofs");
        assert!(
            error.contains(if execution {
                "pending command execution status differs"
            } else {
                "private outbox content/cursor mismatch"
            }),
            "{error}"
        );
        assert_eq!(fingerprint(&directory.0), tampered);
    }
}
