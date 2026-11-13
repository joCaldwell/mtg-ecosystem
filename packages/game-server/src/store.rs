//! Private single-writer durable ledger. No transport acknowledgment is emitted
//! here: callers publish/replace their candidate Game only after commit succeeds.
use crate::codec::exact as strict_read;
use crate::protocol::{
    CommandEnvelope, CommandExecution, CommandExecutionStatus, Counter, ExecutionStatus,
    Publication, RejectionReason, ResumeState, RulesResult, StreamEntry, StreamPage,
    SubmissionResult, checked_publication,
};
use mtg_contracts::{canonical, sha256};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs::{self, File, OpenOptions},
    path::Path,
    time::Duration,
};

pub const MAX_RECORD_BYTES: usize = 131_072;
pub const MAX_VIEW_BYTES: usize = 65_536;
pub const MAX_PUBLICATIONS_PER_SEAT: usize = 2;
const MAX_RECORD_PAGE: u32 = 128;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Head {
    pub sequence: u64,
    pub boundary: u64,
    pub state_sha256: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DurableRecord {
    pub head: Head,
    pub before_boundary: u64,
    pub before_state_sha256: String,
    pub record: Value,
}
pub struct CommandCommit {
    pub seat: u8,
    pub request: CommandEnvelope,
    pub result: SubmissionResult,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionUpdate {
    pub seat: u8,
    pub command_id: String,
    pub status: CommandExecutionStatus,
}
pub struct Transition {
    pub expected: Head,
    pub next_boundary: u64,
    pub state_sha256: String,
    pub record: Value,
    pub command: Option<CommandCommit>,
    pub publications: [Vec<Publication>; 4],
    pub execution_updates: Vec<ExecutionUpdate>,
}
#[derive(Debug, PartialEq, Eq)]
pub enum CommitResult {
    Committed(Head),
    ExistingSubmission(SubmissionResult),
}

pub struct CommittedPublications {
    pub next: Counter,
    pub state: ResumeState,
    pub lifecycle: Option<ExecutionStatus>,
    pub outcome: Option<RulesResult>,
}

pub struct MatchStore {
    connection: Connection,
    _owner_lock: File,
    generation: u64,
}
fn error(e: impl std::fmt::Display) -> String {
    e.to_string()
}
fn hash_valid(hash: &str) -> bool {
    hash.len() == 64
        && hash
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn integer(value: u64) -> Result<i64, String> {
    i64::try_from(value).map_err(|_| "durable sequence exhausted; resource pause required".into())
}
fn head(connection: &Connection) -> Result<Head, String> {
    connection
        .query_row(
            "SELECT sequence,boundary,state_sha FROM head WHERE singleton=1",
            [],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .map_err(error)
        .and_then(|(sequence, boundary, state_sha256)| {
            Ok(Head {
                sequence: u64::try_from(sequence).map_err(error)?,
                boundary: boundary.parse::<u64>().map_err(error)?,
                state_sha256,
            })
        })
}
fn lookup(
    connection: &Connection,
    seat: u8,
    id: &str,
    request: &str,
) -> Result<Option<SubmissionResult>, String> {
    let saved: Option<(String, String)> = connection
        .query_row(
            "SELECT request,result FROM commands WHERE seat=?1 AND command_id=?2",
            params![seat, id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(error)?;
    match saved {
        None => Ok(None),
        Some((prior, result)) if prior == request => strict_read(&result).map(Some),
        Some(_) => Err("idempotency key reused with different command".into()),
    }
}
impl MatchStore {
    pub fn open(directory: &Path, bootstrap: &Value, initial_sha256: &str) -> Result<Self, String> {
        if !hash_valid(initial_sha256) {
            return Err("invalid initial digest".into());
        }
        // This service's initial reference platform is Linux. A persistent file
        // lock is released by the OS after crash; the file is never unlinked.
        // https://doc.rust-lang.org/std/fs/struct.File.html#method.try_lock
        #[cfg(unix)]
        {
            use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
            let mut builder = fs::DirBuilder::new();
            builder.mode(0o700);
            if let Err(e) = builder.create(directory)
                && e.kind() != std::io::ErrorKind::AlreadyExists
            {
                return Err(format!(
                    "pre-existing durable parent directory required: {e}"
                ));
            }
            if fs::metadata(directory).map_err(error)?.permissions().mode() & 0o077 != 0 {
                return Err("match storage directory must be private".into());
            }
        }
        #[cfg(not(unix))]
        {
            return Err("initial durable storage requires Unix private directory semantics".into());
        }
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let owner_lock = options.open(directory.join("owner.lock")).map_err(error)?;
        owner_lock
            .try_lock()
            .map_err(|_| "match store already owned")?;
        let mut connection = Connection::open(directory.join("match.sqlite3")).map_err(error)?;
        connection
            .busy_timeout(Duration::from_millis(100))
            .map_err(error)?;
        // WAL commits are durable only with FULL for the promised ack contract.
        // https://www.sqlite.org/wal.html; https://www.sqlite.org/pragma.html#pragma_synchronous
        connection.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; PRAGMA foreign_keys=ON; PRAGMA max_page_count=262144; PRAGMA wal_autocheckpoint=1000;
CREATE TABLE IF NOT EXISTS metadata(key TEXT PRIMARY KEY,value TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS head(singleton INTEGER PRIMARY KEY CHECK(singleton=1),sequence INTEGER NOT NULL,boundary TEXT NOT NULL,state_sha TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS records(sequence INTEGER PRIMARY KEY,body TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS commands(seat INTEGER NOT NULL CHECK(seat BETWEEN 0 AND 3),command_id TEXT NOT NULL,request TEXT NOT NULL,result TEXT NOT NULL,PRIMARY KEY(seat,command_id));
CREATE TABLE IF NOT EXISTS execution(seat INTEGER NOT NULL CHECK(seat BETWEEN 0 AND 3),command_id TEXT NOT NULL,status TEXT NOT NULL,PRIMARY KEY(seat,command_id),FOREIGN KEY(seat,command_id) REFERENCES commands(seat,command_id));
CREATE TABLE IF NOT EXISTS viewer_heads(seat INTEGER PRIMARY KEY CHECK(seat BETWEEN 0 AND 3),cursor INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS outbox(seat INTEGER NOT NULL CHECK(seat BETWEEN 0 AND 3),cursor INTEGER NOT NULL,body TEXT NOT NULL,PRIMARY KEY(seat,cursor));
CREATE INDEX IF NOT EXISTS outbox_publication_kind ON outbox(seat,json_extract(body,'$.entries[0].publication.kind'),cursor);").map_err(error)?;
        // Existing stores may precede CHECK constraints or have been modified
        // externally. Validate the finite seat domain before generation changes.
        for table in ["commands", "execution", "viewer_heads", "outbox"] {
            let invalid: bool = connection.query_row(
                &format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE typeof(seat) != 'integer' OR seat NOT BETWEEN 0 AND 3)"),
                [], |row| row.get(0),
            ).map_err(error)?;
            if invalid {
                return Err("durable row has invalid seat domain".into());
            }
        }
        let mode: String = connection
            .pragma_query_value(None, "journal_mode", |r| r.get(0))
            .map_err(error)?;
        let sync: u32 = connection
            .pragma_query_value(None, "synchronous", |r| r.get(0))
            .map_err(error)?;
        if mode != "wal" || sync != 2 {
            return Err("durability pragmas did not take effect".into());
        }
        let bootstrap = canonical(&(1_u32, bootstrap, initial_sha256))?;
        if bootstrap.len() > MAX_RECORD_BYTES {
            return Err("bootstrap exceeds bound".into());
        }
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(error)?;
        let previous: Option<String> = tx
            .query_row(
                "SELECT value FROM metadata WHERE key='bootstrap'",
                [],
                |r| r.get(0),
            )
            .optional()
            .map_err(error)?;
        if previous.as_ref().is_some_and(|p| p != &bootstrap) {
            return Err("pinned private bootstrap mismatch".into());
        }
        tx.execute(
            "INSERT OR IGNORE INTO metadata(key,value) VALUES('bootstrap',?1)",
            [bootstrap],
        )
        .map_err(error)?;
        let prior: Option<String> = tx
            .query_row(
                "SELECT value FROM metadata WHERE key='generation'",
                [],
                |r| r.get(0),
            )
            .optional()
            .map_err(error)?;
        let generation = prior
            .map(|s| s.parse::<u64>().map_err(error))
            .transpose()?
            .unwrap_or(0)
            .checked_add(1)
            .ok_or("generation exhausted")?;
        tx.execute("INSERT INTO metadata(key,value) VALUES('generation',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[generation.to_string()]).map_err(error)?;
        tx.execute(
            "INSERT OR IGNORE INTO head VALUES(1,0,'0',?1)",
            [initial_sha256],
        )
        .map_err(error)?;
        for seat in 0..4 {
            tx.execute("INSERT OR IGNORE INTO viewer_heads VALUES(?1,0)", [seat])
                .map_err(error)?;
        }
        tx.commit().map_err(error)?;
        // Persist directory entries in addition to SQLite's own synced writes.
        File::open(directory)
            .and_then(|f| f.sync_all())
            .map_err(error)?;
        // fsync(directory) persists its contents, not its name in the parent.
        // Parent must already exist; this API never creates unsynced ancestors.
        // https://man7.org/linux/man-pages/man2/fsync.2.html
        let parent = directory
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        File::open(parent)
            .and_then(|f| f.sync_all())
            .map_err(error)?;
        Ok(Self {
            connection,
            _owner_lock: owner_lock,
            generation,
        })
    }
    pub fn execution(&self, seat: u8, id: &str) -> Result<Option<CommandExecution>, String> {
        if seat >= 4 {
            return Err("invalid command seat".into());
        }
        let status: Option<String> = self
            .connection
            .query_row(
                "SELECT status FROM execution WHERE seat=?1 AND command_id=?2",
                params![seat, id],
                |row| row.get(0),
            )
            .optional()
            .map_err(error)?;
        status
            .map(|status| {
                strict_read(&status).map(|status| CommandExecution {
                    command_id: id.into(),
                    status,
                })
            })
            .transpose()
    }
    pub fn execution_count(&self, seat: u8) -> Result<u64, String> {
        if seat >= 4 {
            return Err("invalid command seat".into());
        }
        let count: i64 = self
            .connection
            .query_row(
                "SELECT COUNT(*) FROM execution WHERE seat=?1",
                [seat],
                |row| row.get(0),
            )
            .map_err(error)?;
        u64::try_from(count).map_err(error)
    }
    pub fn command_count(&self, seat: u8) -> Result<u64, String> {
        if seat >= 4 {
            return Err("invalid command seat".into());
        }
        let count: i64 = self
            .connection
            .query_row(
                "SELECT COUNT(*) FROM commands WHERE seat=?1",
                [seat],
                |row| row.get(0),
            )
            .map_err(error)?;
        u64::try_from(count).map_err(error)
    }
    pub fn viewer_cursor(&self, seat: u8) -> Result<u64, String> {
        if seat >= 4 {
            return Err("invalid viewer seat".into());
        }
        let cursor: i64 = self
            .connection
            .query_row(
                "SELECT cursor FROM viewer_heads WHERE seat=?1",
                [seat],
                |row| row.get(0),
            )
            .map_err(error)?;
        u64::try_from(cursor).map_err(error)
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn head(&self) -> Result<Head, String> {
        head(&self.connection)
    }
    pub fn existing_submission(
        &self,
        seat: u8,
        request: &CommandEnvelope,
    ) -> Result<Option<SubmissionResult>, String> {
        if seat >= 4 {
            return Err("invalid seat".into());
        }
        lookup(
            &self.connection,
            seat,
            &request.command_id,
            &canonical(request)?,
        )
    }
    pub fn commit(
        &mut self,
        generation: u64,
        transition: Transition,
    ) -> Result<CommitResult, String> {
        if generation != self.generation {
            return Err("stale worker generation".into());
        }
        if !hash_valid(&transition.state_sha256)
            || transition.next_boundary < transition.expected.boundary
        {
            return Err("invalid durable boundary".into());
        }
        if let Some(command) = &transition.command
            && let Some(result) = self.existing_submission(command.seat, &command.request)?
        {
            return Ok(CommitResult::ExistingSubmission(result));
        }
        let sequence = transition
            .expected
            .sequence
            .checked_add(1)
            .ok_or("durable sequence exhausted")?;
        let next = Head {
            sequence,
            boundary: transition.next_boundary,
            state_sha256: transition.state_sha256,
        };
        let record = canonical(&DurableRecord {
            head: next.clone(),
            before_boundary: transition.expected.boundary,
            before_state_sha256: transition.expected.state_sha256.clone(),
            record: transition.record,
        })?;
        if record.len() > MAX_RECORD_BYTES {
            return Err("durable record exceeds bound".into());
        }
        let command = transition
            .command
            .map(|command| -> Result<_, String> {
                if command.seat >= 4 {
                    return Err("invalid command seat".into());
                }
                let id = match &command.result {
                    SubmissionResult::Accepted { command_id }
                    | SubmissionResult::Rejected { command_id, .. } => command_id,
                };
                if id != &command.request.command_id {
                    return Err("submission ID mismatch".into());
                }
                Ok((
                    command.seat,
                    command.request.command_id.clone(),
                    canonical(&command.request)?,
                    canonical(&command.result)?,
                ))
            })
            .transpose()?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(error)?;
        let persisted_generation: String = tx
            .query_row(
                "SELECT value FROM metadata WHERE key='generation'",
                [],
                |r| r.get(0),
            )
            .map_err(error)?;
        if persisted_generation != generation.to_string() || head(&tx)? != transition.expected {
            return Err("stale durable state or worker generation".into());
        }
        tx.execute(
            "INSERT INTO records VALUES(?1,?2)",
            params![integer(sequence)?, record],
        )
        .map_err(error)?;
        for (seat, publications) in transition.publications.into_iter().enumerate() {
            let seat = u8::try_from(seat).map_err(error)?;
            if publications.len() > MAX_PUBLICATIONS_PER_SEAT {
                return Err("outbox transition bound".into());
            }
            let mut cursor: i64 = tx
                .query_row(
                    "SELECT cursor FROM viewer_heads WHERE seat=?1",
                    [seat],
                    |r| r.get(0),
                )
                .map_err(error)?;
            for publication in publications {
                if matches!(&publication, Publication::ViewReplaced { observation } if observation.view.viewer.0 != seat)
                {
                    return Err("projection viewer does not match private outbox seat".into());
                }
                cursor = cursor.checked_add(1).ok_or("viewer cursor exhausted")?;
                let entry = StreamEntry {
                    cursor: Counter::new(u64::try_from(cursor).map_err(error)?),
                    publication,
                };
                let page = StreamPage {
                    protocol_version: 1,
                    entries: vec![entry],
                    next: Counter::new(u64::try_from(cursor).map_err(error)?),
                    has_more: false,
                };
                let body = checked_publication(&page)?;
                if body.len() > MAX_VIEW_BYTES {
                    return Err("outbox entry exceeds bound".into());
                }
                tx.execute(
                    "INSERT INTO outbox VALUES(?1,?2,?3)",
                    params![seat, cursor, body],
                )
                .map_err(error)?;
            }
            tx.execute(
                "UPDATE viewer_heads SET cursor=?2 WHERE seat=?1",
                params![seat, cursor],
            )
            .map_err(error)?;
        }
        if let Some((seat, id, request, result)) = command {
            let submission: SubmissionResult = strict_read(&result)?;
            let initial = match submission {
                SubmissionResult::Accepted { .. } => CommandExecutionStatus::Pending,
                SubmissionResult::Rejected {
                    reason: RejectionReason::Cancelled,
                    ..
                } => CommandExecutionStatus::Cancelled,
                SubmissionResult::Rejected { reason, .. } => {
                    CommandExecutionStatus::Rejected { reason }
                }
            };
            tx.execute(
                "INSERT INTO commands VALUES(?1,?2,?3,?4)",
                params![seat, id, request, result],
            )
            .map_err(error)?;
            tx.execute(
                "INSERT INTO execution VALUES(?1,?2,?3)",
                params![seat, id, canonical(&initial)?],
            )
            .map_err(error)?;
        }
        if transition.execution_updates.len() > 4 {
            return Err("execution update bound".into());
        }
        let mut updated = std::collections::BTreeSet::new();
        for update in transition.execution_updates {
            if update.seat >= 4
                || !updated.insert((update.seat, update.command_id.clone()))
                || !matches!(
                    update.status,
                    CommandExecutionStatus::ConsumedByRules
                        | CommandExecutionStatus::DiscardedByDeparture
                )
            {
                return Err("invalid execution status update".into());
            }
            let (current, submission): (String, String) = tx
                .query_row(
                    "SELECT execution.status,commands.result FROM execution JOIN commands USING(seat,command_id) WHERE seat=?1 AND command_id=?2",
                    params![update.seat, update.command_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .map_err(error)?;
            if !matches!(strict_read::<SubmissionResult>(&submission)?, SubmissionResult::Accepted { command_id } if command_id == update.command_id)
            {
                return Err("execution update requires matching immutable acceptance".into());
            }
            if strict_read::<CommandExecutionStatus>(&current)? != CommandExecutionStatus::Pending {
                return Err("execution disposition is already final".into());
            }
            tx.execute(
                "UPDATE execution SET status=?3 WHERE seat=?1 AND command_id=?2",
                params![update.seat, update.command_id, canonical(&update.status)?],
            )
            .map_err(error)?;
        }
        tx.execute(
            "UPDATE head SET sequence=?1,boundary=?2,state_sha=?3 WHERE singleton=1",
            params![
                integer(sequence)?,
                next.boundary.to_string(),
                next.state_sha256
            ],
        )
        .map_err(error)?;
        tx.commit().map_err(error)?;
        Ok(CommitResult::Committed(next))
    }
    pub fn stream_page(&self, seat: u8, after: u64, limit: u32) -> Result<StreamPage, String> {
        if seat >= 4 || !(1..=32).contains(&limit) {
            return Err("invalid private page bound".into());
        }
        let latest: i64 = self
            .connection
            .query_row(
                "SELECT cursor FROM viewer_heads WHERE seat=?1",
                [seat],
                |r| r.get(0),
            )
            .map_err(error)?;
        if after > u64::try_from(latest).map_err(error)? {
            return Err("cursor exceeds viewer history".into());
        }
        let mut query = self
            .connection
            .prepare("SELECT body FROM outbox WHERE seat=?1 AND cursor>?2 ORDER BY cursor LIMIT ?3")
            .map_err(error)?;
        let rows = query
            .query_map(params![seat, integer(after)?, limit], |r| {
                r.get::<_, String>(0)
            })
            .map_err(error)?;
        let mut entries = vec![];
        for row in rows {
            let page: StreamPage = strict_read(&row.map_err(error)?)?;
            entries.extend(page.entries);
        }
        let next = entries
            .last()
            .map(|e| e.cursor.clone())
            .unwrap_or_else(|| Counter::new(after));
        Ok(StreamPage {
            protocol_version: 1,
            has_more: next.get() < u64::try_from(latest).map_err(error)?,
            entries,
            next,
        })
    }
    /// Indexed seeks in a single SQLite read snapshot; cost does not grow with
    /// the number of pages a reconnecting browser missed. Returns public data only.
    pub fn resume_publications(&self, seat: u8) -> Result<CommittedPublications, String> {
        if seat >= 4 {
            return Err("invalid resume seat".into());
        }
        let tx = self.connection.unchecked_transaction().map_err(error)?;
        let latest: i64 = tx
            .query_row(
                "SELECT cursor FROM viewer_heads WHERE seat=?1",
                [seat],
                |r| r.get(0),
            )
            .map_err(error)?;
        let next = Counter::new(u64::try_from(latest).map_err(error)?);
        let mut state = ResumeState::Pending;
        let mut lifecycle = None;
        let mut outcome = None;
        for kind in ["view_replaced", "lifecycle", "outcome"] {
            // SQL bounds before materializing a corrupt record. The expression
            // matches outbox_publication_kind exactly (SQLite expression index).
            let row: Option<(i64, Option<String>)> = tx.query_row(
                "SELECT cursor,CASE WHEN typeof(body)='text' AND length(CAST(body AS BLOB))<=?4 THEN body ELSE NULL END FROM outbox WHERE seat=?1 AND json_extract(body,'$.entries[0].publication.kind')=?2 AND cursor<=?3 ORDER BY cursor DESC LIMIT 1",
                params![seat, kind, latest, MAX_VIEW_BYTES as i64], |r| Ok((r.get(0)?,r.get(1)?))
            ).optional().map_err(error)?;
            if let Some((cursor, body)) = row {
                let page: StreamPage =
                    strict_read(&body.ok_or("resume publication exceeds bound")?)?;
                if page.protocol_version != 1
                    || page.entries.len() != 1
                    || page.has_more
                    || page.next.get() != u64::try_from(cursor).map_err(error)?
                    || page.entries[0].cursor != page.next
                {
                    return Err("invalid resume publication".into());
                }
                match page
                    .entries
                    .into_iter()
                    .next()
                    .expect("checked one entry")
                    .publication
                {
                    Publication::ViewReplaced { observation }
                        if kind == "view_replaced" && observation.view.viewer.0 == seat =>
                    {
                        state = ResumeState::Available { observation }
                    }
                    Publication::Lifecycle { status } if kind == "lifecycle" => {
                        lifecycle = Some(status)
                    }
                    Publication::Outcome { result } if kind == "outcome" => outcome = Some(result),
                    _ => return Err("invalid resume publication kind or viewer".into()),
                }
            }
        }
        tx.commit().map_err(error)?;
        Ok(CommittedPublications {
            next,
            state,
            lifecycle,
            outcome,
        })
    }
    pub fn records_after(&self, after: u64, limit: u32) -> Result<Vec<DurableRecord>, String> {
        if !(1..=MAX_RECORD_PAGE).contains(&limit) {
            return Err("invalid recovery page bound".into());
        }
        let mut query = self
            .connection
            // SQLite length(TEXT) counts code points and stops at NUL, whereas
            // length(BLOB) counts bytes: https://www.sqlite.org/lang_corefunc.html#length
            // Gate inside SQL before rusqlite materializes a corrupt body String.
            .prepare("SELECT CASE WHEN typeof(body)='text' AND length(CAST(body AS BLOB))<=?3 THEN body ELSE NULL END FROM records WHERE sequence>?1 ORDER BY sequence LIMIT ?2")
            .map_err(error)?;
        let rows = query
            .query_map(
                params![
                    integer(after)?,
                    limit,
                    i64::try_from(MAX_RECORD_BYTES).map_err(error)?
                ],
                |r| r.get::<_, Option<String>>(0),
            )
            .map_err(error)?;
        rows.map(|row| {
            let body = row
                .map_err(error)?
                .ok_or("recovery record type or byte bound")?;
            strict_read(&body)
        })
        .collect()
    }
    pub fn bootstrap_sha256(&self) -> Result<String, String> {
        let json: String = self
            .connection
            .query_row(
                "SELECT value FROM metadata WHERE key='bootstrap'",
                [],
                |r| r.get(0),
            )
            .map_err(error)?;
        Ok(sha256(json.as_bytes()))
    }
}
