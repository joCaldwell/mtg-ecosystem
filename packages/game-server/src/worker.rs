//! Single-owner durable core executor. These records are host-private, not wire
//! DTOs. A candidate is visible only after its state/idempotency/outbox commit.
use crate::codec::exact as strict_read;
use crate::protocol::{
    CommandBody, CommandEnvelope, CommandExecutionStatus, Counter, ExecutionStatus, Publication,
    RejectionReason, RulesResult, StreamPage, SubmissionResult, checked_publication,
};
use crate::recovery_hashes::{AfterHashJob, AfterHashPool, BATCH_SIZE};
use crate::store::{
    CommandCommit, CommitResult, DurableRecord, ExecutionUpdate, Head, MAX_RECORD_BYTES,
    MAX_VIEW_BYTES, MatchStore, Transition,
};
use mtg_contracts::{Catalog, canonical, digest, sha256};
use mtg_core::{
    Advance, Budget, CoreInputIdentity, Game, InputProcessingCause, InputProcessingFact,
    MatchConfig, Observation, PlayerId, PlayerResponse, Prepared, QueryResult, QueryValue,
    TraceMode,
};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::Path,
    time::Instant,
};

const VERSION: u32 = 3;
const QUANTUM: u64 = 64;
const QUERY_LIMIT: u64 = 100_000;
const INITIAL_CHECKPOINT_BYTES: u64 = 4 * 1024 * 1024;
const INITIAL_CLOCK_NS: u64 = 30 * 60 * 1_000_000_000;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "phase", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkerPhase {
    Running,
    Waiting,
    Paused { reason: PauseReason },
    Faulted,
    Finished { result: RulesResult },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PauseReason {
    Operator,
    ClockExpired,
    QueryLimit,
    PublicationLimit,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PendingCommand {
    pub seat: u8,
    pub command_id: String,
    pub identity: CoreInputIdentity,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostState {
    pub phase: WorkerPhase,
    pub remaining_ns: [u64; 4],
    pub ready: [bool; 4],
    pub viewer_cursors: [u64; 4],
    pub observation_hashes: [Option<String>; 4],
    pub outstanding: Vec<PendingCommand>,
}
impl Default for HostState {
    fn default() -> Self {
        Self {
            phase: WorkerPhase::Running,
            remaining_ns: [INITIAL_CLOCK_NS; 4],
            ready: [false; 4],
            viewer_cursors: [0; 4],
            observation_hashes: std::array::from_fn(|_| None),
            outstanding: vec![],
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "stop", rename_all = "snake_case", deny_unknown_fields)]
pub enum AdvanceStop {
    Suspended,
    Awaiting { player: PlayerId, decision_id: u64 },
    Win { player: PlayerId },
    Draw,
    Fault { reason: String },
}
impl AdvanceStop {
    fn from_advance(value: Advance) -> Self {
        match value {
            Advance::Suspended { .. } => Self::Suspended,
            Advance::AwaitingDecision(r) => Self::Awaiting {
                player: r.player(),
                decision_id: r.decision_id(),
            },
            Advance::Finished { winner } => Self::Win { player: winner },
            Advance::Draw => Self::Draw,
            Advance::Fault(reason) => Self::Fault { reason },
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum CoreOperation {
    Advance {
        budget: u64,
        expected_return: AdvanceStop,
        processed: Vec<InputProcessingFact>,
    },
    Decision {
        seat: PlayerId,
        token: mtg_core::DecisionToken,
        response: PlayerResponse,
    },
    Concede {
        seat: PlayerId,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CommandStamp {
    protocol_version: u32,
    command_id: String,
    expected_revision: Counter,
}
impl CommandStamp {
    fn from_request(r: &CommandEnvelope) -> Self {
        Self {
            protocol_version: r.protocol_version,
            command_id: r.command_id.clone(),
            expected_revision: r.expected_revision.clone(),
        }
    }
    fn request(&self, operation: &CoreOperation) -> Result<(u8, CommandEnvelope), String> {
        let (seat, command) = match operation {
            CoreOperation::Decision {
                seat,
                token,
                response,
            } => (
                seat.0,
                CommandBody::Decision {
                    token: token.clone(),
                    response: response.clone(),
                },
            ),
            CoreOperation::Concede { seat } => (seat.0, CommandBody::Concede),
            CoreOperation::Advance { .. } => {
                return Err("advance cannot have a command stamp".into());
            }
        };
        Ok((
            seat,
            CommandEnvelope {
                protocol_version: self.protocol_version,
                command_id: self.command_id.clone(),
                expected_revision: self.expected_revision.clone(),
                command,
            },
        ))
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
enum Action {
    Core {
        operation: CoreOperation,
        command: Option<CommandStamp>,
    },
    Rejected {
        seat: u8,
        request: CommandEnvelope,
        reason: RejectionReason,
    },
    Cancelled {
        seat: u8,
        request: CommandEnvelope,
    },
    Clock {
        control: ClockControl,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ClockControl {
    Tick,
    Pause,
    Resume,
    ResourcePause { reason: PauseReason },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkerRecord {
    worker_record_version: u32,
    elapsed_ns: u64,
    before_host_sha256: String,
    action: Action,
    after_host: HostState,
    publication_hashes: [Vec<String>; 4],
    execution_updates: Vec<ExecutionUpdate>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Bootstrap {
    worker_version: u32,
    match_id: String,
    engine_build_id: String,
    server_build_id: String,
    catalog_sha256: String,
    rules_sha256: String,
    commander_policy_sha256: String,
    config: MatchConfig,
    checkpoint_sha256: String,
    initial_state_sha256: String,
    initial_clock_ns: u64,
}

struct CandidateState {
    game: Game,
    host: HostState,
    execution_updates: Vec<ExecutionUpdate>,
}

/// Actual prepared executor identity. Host-private: profile hash binds the seed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerIdentity {
    pub match_id: String,
    pub engine_build_id: String,
    pub server_build_id: String,
    pub catalog_sha256: String,
    pub profile_sha256: String,
}
impl WorkerIdentity {
    fn from_prepared(prepared: &Prepared, match_id: &str) -> Self {
        Self {
            match_id: match_id.into(),
            engine_build_id: mtg_core::BUILD_ID.into(),
            server_build_id: crate::SERVER_BUILD_ID.to_string(),
            catalog_sha256: prepared.catalog_digest().into(),
            profile_sha256: prepared.profile_digest().into(),
        }
    }
}
pub struct Worker {
    identity: WorkerIdentity,
    decoder: crate::protocol::CommandDecoder,
    game: Game,
    store: MatchStore,
    head: Head,
    host: HostState,
    last_sample: Option<Instant>,
    halted: bool,
}

fn bounded_read(path: &Path, limit: u64) -> Result<String, String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    if file.metadata().map_err(|e| e.to_string())?.len() > limit {
        return Err("private checkpoint/bootstrap exceeds bound".into());
    }
    let mut bytes = vec![];
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > limit {
        return Err("private file grew beyond bound".into());
    }
    String::from_utf8(bytes).map_err(|e| e.to_string())
}
fn private_file(directory: &Path, name: &str, body: &[u8]) -> Result<(), String> {
    let temporary = directory.join(format!(".{name}.pending"));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&temporary).map_err(|e| e.to_string())?;
    file.write_all(body)
        .and_then(|()| file.sync_all())
        .map_err(|e| e.to_string())?;
    fs::rename(temporary, directory.join(name)).map_err(|e| e.to_string())?;
    File::open(directory)
        .and_then(|f| f.sync_all())
        .map_err(|e| e.to_string())
}
fn query(game: &Game, mut result: QueryResult) -> Result<QueryValue, String> {
    let mut calls = 1;
    loop {
        match result {
            QueryResult::Complete(value) => return Ok(value),
            QueryResult::Rejected(reason) => return Err(format!("query rejected: {reason:?}")),
            QueryResult::Incomplete {
                work,
                required_minimum,
            } => {
                if required_minimum > QUANTUM || calls >= QUERY_LIMIT {
                    return Err("query resource bound".into());
                }
                calls += 1;
                result = game.resume_work(work, Budget::new(QUANTUM));
            }
        }
    }
}
fn observe(game: &Game, seat: u8) -> Result<Observation, String> {
    match query(game, game.observe(PlayerId(seat), Budget::new(QUANTUM)))? {
        QueryValue::Observation(value) => Ok(value),
        _ => Err("observation query returned wrong value".into()),
    }
}
fn debit(host: &mut HostState, elapsed: u64) {
    if !matches!(host.phase, WorkerPhase::Waiting) {
        return;
    }
    for (ready, remaining) in host.ready.iter().zip(&mut host.remaining_ns) {
        if *ready {
            *remaining = remaining.saturating_sub(elapsed);
        }
    }
    if host
        .ready
        .iter()
        .zip(host.remaining_ns)
        .any(|(ready, left)| *ready && left == 0)
    {
        host.phase = WorkerPhase::Paused {
            reason: PauseReason::ClockExpired,
        };
    }
}
fn execution(host: &HostState) -> ExecutionStatus {
    match host.phase {
        WorkerPhase::Running => ExecutionStatus::Running,
        WorkerPhase::Waiting => ExecutionStatus::Waiting,
        WorkerPhase::Paused { .. } => ExecutionStatus::ResourcePaused,
        WorkerPhase::Faulted => ExecutionStatus::Faulted,
        WorkerPhase::Finished { .. } => ExecutionStatus::Finished,
    }
}
fn publications(
    game: &Game,
    before: &HostState,
    after: &mut HostState,
    coherent: bool,
    terminal: Option<RulesResult>,
) -> Result<[Vec<Publication>; 4], String> {
    let mut output: [Vec<Publication>; 4] = std::array::from_fn(|_| vec![]);
    if coherent {
        after.ready = [false; 4];
        for seat in 0..4u8 {
            let view = observe(game, seat)?;
            let index = usize::from(seat);
            after.ready[index] = view.decision.is_some();
            let hash = digest(&view)?;
            if before.observation_hashes[index].as_ref() != Some(&hash) {
                after.observation_hashes[index] = Some(hash);
                output[index].push(Publication::ViewReplaced {
                    observation: Box::new(view),
                });
            }
        }
    }
    if let Some(result) = terminal {
        if !matches!(before.phase, WorkerPhase::Finished { .. }) {
            for row in &mut output {
                row.push(Publication::Outcome {
                    result: result.clone(),
                });
            }
        }
    } else if (matches!(
        after.phase,
        WorkerPhase::Paused { .. } | WorkerPhase::Faulted
    ) || matches!(before.phase, WorkerPhase::Paused { .. }))
        && execution(before) != execution(after)
    {
        for row in &mut output {
            row.push(Publication::Lifecycle {
                status: execution(after),
            });
        }
    }
    for (index, row) in output.iter().enumerate() {
        for publication in row {
            after.viewer_cursors[index] = after.viewer_cursors[index]
                .checked_add(1)
                .ok_or("private cursor exhausted")?;
            let page = StreamPage {
                protocol_version: 1,
                entries: vec![crate::protocol::StreamEntry {
                    cursor: Counter::new(after.viewer_cursors[index]),
                    publication: publication.clone(),
                }],
                next: Counter::new(after.viewer_cursors[index]),
                has_more: false,
            };
            if checked_publication(&page)?.len() > MAX_VIEW_BYTES {
                return Err("publication resource bound".into());
            }
        }
    }
    Ok(output)
}
fn finish_advance(host: &mut HostState, stop: &AdvanceStop) -> (bool, Option<RulesResult>) {
    match stop {
        AdvanceStop::Suspended => {
            host.phase = WorkerPhase::Running;
            host.ready = [false; 4];
            (false, None)
        }
        AdvanceStop::Awaiting { .. } => {
            host.phase = WorkerPhase::Waiting;
            (true, None)
        }
        AdvanceStop::Win { player } => {
            host.phase = WorkerPhase::Finished {
                result: RulesResult::Win { player: *player },
            };
            host.ready = [false; 4];
            (true, Some(RulesResult::Win { player: *player }))
        }
        AdvanceStop::Draw => {
            host.phase = WorkerPhase::Finished {
                result: RulesResult::Draw,
            };
            host.ready = [false; 4];
            (true, Some(RulesResult::Draw))
        }
        AdvanceStop::Fault { .. } => {
            host.phase = WorkerPhase::Faulted;
            host.ready = [false; 4];
            (false, None)
        }
    }
}

pub(crate) fn valid_match_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
impl Worker {
    pub fn create(
        directory: &Path,
        catalog: Catalog,
        config: MatchConfig,
        match_id: &str,
        now: Instant,
    ) -> Result<Self, String> {
        if !valid_match_id(match_id) {
            return Err("invalid worker match ID".into());
        }
        let busy = Instant::now();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            fs::DirBuilder::new()
                .mode(0o700)
                .create(directory)
                .map_err(|e| e.to_string())?;
        }
        #[cfg(not(unix))]
        {
            return Err("worker storage requires Unix private directories".into());
        }
        let prepared = Prepared::admit_pod(catalog.clone(), config.clone())
            .map_err(|e| format!("match admission: {e:?}"))?;
        let identity = WorkerIdentity::from_prepared(&prepared, match_id);
        let game = Game::with_trace(prepared, TraceMode::Off);
        let (checkpoint, initial_state_sha256) = game.snapshot_with_digest()?;
        if checkpoint.len() as u64 > INITIAL_CHECKPOINT_BYTES {
            return Err("initial checkpoint exceeds bound".into());
        }
        let bootstrap = Bootstrap {
            worker_version: VERSION,
            match_id: match_id.into(),
            engine_build_id: mtg_core::BUILD_ID.into(),
            server_build_id: crate::SERVER_BUILD_ID.to_string(),
            catalog_sha256: digest(&catalog)?,
            rules_sha256: catalog.rules_sha256.clone(),
            commander_policy_sha256: mtg_core::policy_digest(),
            config,
            checkpoint_sha256: sha256(checkpoint.as_bytes()),
            initial_state_sha256: initial_state_sha256.clone(),
            initial_clock_ns: INITIAL_CLOCK_NS,
        };
        let bootstrap_text = canonical(&bootstrap)?;
        if bootstrap_text.len() > MAX_RECORD_BYTES {
            return Err("worker bootstrap exceeds bound".into());
        }
        private_file(directory, "checkpoint0.json", checkpoint.as_bytes())?;
        private_file(directory, "bootstrap.json", bootstrap_text.as_bytes())?;
        let store = MatchStore::open(
            directory,
            &serde_json::to_value(&bootstrap).map_err(|e| e.to_string())?,
            &initial_state_sha256,
        )?;
        let head = store.head()?;
        Ok(Self {
            decoder: crate::protocol::CommandDecoder::new()?,
            identity,
            game,
            store,
            head,
            host: HostState::default(),
            last_sample: Some(
                now.checked_add(busy.elapsed())
                    .ok_or("clock anchor overflow")?,
            ),
            halted: false,
        })
    }
    pub fn recover(
        directory: &Path,
        catalog: Catalog,
        match_id: &str,
        now: Instant,
    ) -> Result<Self, String> {
        if !valid_match_id(match_id) {
            return Err("invalid worker match ID".into());
        }
        let busy = Instant::now();
        let bootstrap: Bootstrap = strict_read(&bounded_read(
            &directory.join("bootstrap.json"),
            MAX_RECORD_BYTES as u64,
        )?)?;
        if bootstrap.worker_version != VERSION
            || bootstrap.match_id != match_id
            || bootstrap.engine_build_id != mtg_core::BUILD_ID
            || bootstrap.server_build_id != *crate::SERVER_BUILD_ID
            || bootstrap.catalog_sha256 != digest(&catalog)?
            || bootstrap.rules_sha256 != catalog.rules_sha256
            || bootstrap.commander_policy_sha256 != mtg_core::policy_digest()
            || bootstrap.initial_clock_ns != INITIAL_CLOCK_NS
        {
            return Err("worker bootstrap identity mismatch".into());
        }
        let checkpoint = bounded_read(
            &directory.join("checkpoint0.json"),
            INITIAL_CHECKPOINT_BYTES,
        )?;
        if sha256(checkpoint.as_bytes()) != bootstrap.checkpoint_sha256 {
            return Err("initial checkpoint file hash mismatch".into());
        }
        let prepared = Prepared::admit_pod(catalog, bootstrap.config.clone())
            .map_err(|e| format!("match admission: {e:?}"))?;
        let identity = WorkerIdentity::from_prepared(&prepared, &bootstrap.match_id);
        let game = Game::restore(prepared, &checkpoint)?;
        if game.rule_boundary() != 0
            || game.state_digest() != Ok(bootstrap.initial_state_sha256.clone())
        {
            return Err("initial checkpoint is not boundary zero".into());
        }
        let store = MatchStore::open(
            directory,
            &serde_json::to_value(&bootstrap).map_err(|e| e.to_string())?,
            &bootstrap.initial_state_sha256,
        )?;
        let durable_head = store.head()?;
        let decoder = crate::protocol::CommandDecoder::new()?;
        // This cursor is speculative, private and unusable by any live Worker,
        // IPC or policy. Its expected head is NOT a verified digest cache while
        // a batch is in flight. No Worker is constructed until every actual
        // after-State hash, every ledger proof and every global check passes.
        let mut cursor = ReplayCursor {
            game,
            host: HostState::default(),
            expected_head: Head {
                sequence: 0,
                boundary: 0,
                state_sha256: bootstrap.initial_state_sha256,
            },
        };
        let mut hashes = AfterHashPool::new()?;
        let mut commands = std::collections::BTreeSet::new();
        let page_size = u32::try_from(BATCH_SIZE).map_err(|_| "recovery page bound")?;
        while cursor.expected_head.sequence < durable_head.sequence {
            let page = store.records_after(cursor.expected_head.sequence, page_size)?;
            if page.is_empty() {
                return Err("durable operation sequence is incomplete".into());
            }
            for record in page {
                if record.head.sequence > durable_head.sequence {
                    return Err("operation exceeds durable head".into());
                }
                // Six total outstanding snapshots includes active, queued and
                // completed-but-undrained jobs. The core's deepest simultaneous
                // advance/event path adds four temporary Games: worst-case is
                // eleven Game values, plus three history-free digest skeleton
                // State clones. Actual total RSS remains capped externally.
                let (job, key) = cursor.stage_record(&store, &decoder, record)?;
                hashes.enqueue(job)?;
                if let Some(key) = key
                    && !commands.insert(key)
                {
                    return Err("duplicate command operation in durable ledger".into());
                }
            }
            // Never stage record seven before EVERY preceding actual digest
            // matches. EOF partial pages receive the same complete barrier.
            hashes.barrier()?;
        }
        hashes.finish()?;
        if cursor.expected_head != durable_head {
            return Err("recovered worker head differs from store".into());
        }
        if !store.records_after(durable_head.sequence, 1)?.is_empty() {
            return Err("orphan operations beyond durable head".into());
        }
        for seat in 0..4u8 {
            let expected_commands = commands.iter().filter(|(owner, _)| *owner == seat).count();
            if store.command_count(seat)?
                != u64::try_from(expected_commands).map_err(|_| "command count overflow")?
                || store.execution_count(seat)? != store.command_count(seat)?
            {
                return Err("orphan command idempotency rows".into());
            }
            if !store
                .stream_page(seat, cursor.host.viewer_cursors[usize::from(seat)], 1)?
                .entries
                .is_empty()
            {
                return Err("orphan private outbox rows".into());
            }
            if store.viewer_cursor(seat)? != cursor.host.viewer_cursors[usize::from(seat)] {
                return Err("private outbox head mismatch".into());
            }
        }
        for pending in &cursor.host.outstanding {
            if store.execution(pending.seat, &pending.command_id)?
                != Some(crate::protocol::CommandExecution {
                    command_id: pending.command_id.clone(),
                    status: CommandExecutionStatus::Pending,
                })
            {
                return Err("pending command execution status differs".into());
            }
        }
        // Downtime/recovery time is not charged. Thread startup, verification
        // barriers and joins are part of the excluded recovery interval.
        let last_sample = Some(
            now.checked_add(busy.elapsed())
                .ok_or("clock anchor overflow")?,
        );
        Ok(Self {
            decoder,
            identity,
            game: cursor.game,
            store,
            head: cursor.expected_head,
            host: cursor.host,
            last_sample,
            halted: false,
        })
    }
    /// Identity comes from the exact Prepared passed to Game creation/restore,
    /// never an expected registry value or an unvalidated caller assertion.
    pub fn identity(&self) -> &WorkerIdentity {
        &self.identity
    }
    pub fn head(&self) -> &Head {
        &self.head
    }
    pub fn host_state(&self) -> &HostState {
        &self.host
    }
    pub fn state_sha256(&self) -> Result<String, String> {
        self.game.state_digest()
    }
    pub fn stream_page(&self, seat: u8, after: u64, limit: u32) -> Result<StreamPage, String> {
        self.store.stream_page(seat, after, limit)
    }
    /// Host authenticates `seat`; no caller-supplied viewer may cross seats.
    pub fn command_execution(
        &self,
        seat: u8,
        command_id: &str,
    ) -> Result<Option<crate::protocol::CommandExecution>, String> {
        self.store.execution(seat, command_id)
    }
    fn elapsed(&self, now: Instant) -> Result<u64, String> {
        let Some(previous) = self.last_sample else {
            return Ok(0);
        };
        u64::try_from(
            now.checked_duration_since(previous)
                .ok_or("nonmonotonic worker clock")?
                .as_nanos(),
        )
        .map_err(|_| "worker clock elapsed overflow".into())
    }
    fn commit(
        &mut self,
        candidate: CandidateState,
        action: Action,
        elapsed: u64,
        pubs: [Vec<Publication>; 4],
        command: Option<CommandCommit>,
        now: Instant,
    ) -> Result<(), String> {
        let CandidateState {
            game: candidate,
            host,
            execution_updates,
        } = candidate;
        let hashes: [Result<Vec<String>, String>; 4] = std::array::from_fn(|seat| {
            pubs[seat].iter().map(digest).collect::<Result<Vec<_>, _>>()
        });
        let mut publication_hashes: [Vec<String>; 4] = std::array::from_fn(|_| vec![]);
        for (index, row) in hashes.into_iter().enumerate() {
            publication_hashes[index] = row?;
        }
        let body = WorkerRecord {
            worker_record_version: VERSION,
            elapsed_ns: elapsed,
            before_host_sha256: digest(&self.host)?,
            action,
            after_host: host.clone(),
            publication_hashes,
            execution_updates: execution_updates.clone(),
        };
        let record = serde_json::to_value(body).map_err(|e| e.to_string())?;
        let state_sha256 = candidate.state_digest()?;
        let wrapped = DurableRecord {
            head: Head {
                sequence: self
                    .head
                    .sequence
                    .checked_add(1)
                    .ok_or("record sequence exhausted")?,
                boundary: candidate.rule_boundary(),
                state_sha256: state_sha256.clone(),
            },
            before_boundary: self.head.boundary,
            before_state_sha256: self.head.state_sha256.clone(),
            record: record.clone(),
        };
        if canonical(&wrapped)?.len() > MAX_RECORD_BYTES {
            return Err("worker record resource bound".into());
        }
        let transition = Transition {
            expected: self.head.clone(),
            next_boundary: candidate.rule_boundary(),
            state_sha256,
            record,
            command,
            publications: pubs,
            execution_updates,
        };
        match self.store.commit(self.store.generation(), transition) {
            Ok(CommitResult::Committed(head)) => {
                self.game = candidate;
                self.host = host;
                self.head = head;
                self.last_sample = Some(now);
                Ok(())
            }
            Ok(CommitResult::ExistingSubmission(_)) => {
                Err("unexpected concurrent duplicate inside sole worker".into())
            }
            Err(reason) => {
                self.halted = true;
                Err(format!("durable commit failed; worker halted: {reason}"))
            }
        }
    }
    fn reject(
        &mut self,
        seat: u8,
        request: CommandEnvelope,
        reason: RejectionReason,
        mut host: HostState,
        elapsed: u64,
        now: Instant,
    ) -> Result<SubmissionResult, String> {
        let result = SubmissionResult::Rejected {
            command_id: request.command_id.clone(),
            reason,
        };
        let pubs = publications(&self.game, &self.host, &mut host, false, None)?;
        self.commit(
            CandidateState {
                game: self.game.fork(),
                host,
                execution_updates: vec![],
            },
            Action::Rejected {
                seat,
                request: request.clone(),
                reason,
            },
            elapsed,
            pubs,
            Some(CommandCommit {
                seat,
                request,
                result: result.clone(),
            }),
            now,
        )?;
        Ok(result)
    }
    pub fn submit(
        &mut self,
        seat: u8,
        request: CommandEnvelope,
        now: Instant,
    ) -> Result<SubmissionResult, String> {
        let busy = Instant::now();
        let result = self.submit_inner(seat, request, now);
        let duration = busy.elapsed();
        if let Some(anchor) = self.last_sample {
            self.last_sample = Some(
                anchor
                    .checked_add(duration)
                    .ok_or("clock anchor overflow")?,
            );
        }
        result
    }
    fn submit_inner(
        &mut self,
        seat: u8,
        request: CommandEnvelope,
        now: Instant,
    ) -> Result<SubmissionResult, String> {
        if self.halted {
            return Err("worker is halted".into());
        }
        if seat >= 4 {
            return Err("invalid authenticated seat".into());
        }
        self.decoder.decode(&canonical(&request)?)?;
        // Duplicate immutable outcomes win over stale viewer revisions and
        // clock expiry. A conflicting body fails before touching the Game.
        if let Some(result) = self.store.existing_submission(seat, &request)? {
            return Ok(result);
        }
        let elapsed = self.elapsed(now)?;
        let mut host = self.host.clone();
        debit(&mut host, elapsed);
        let reason = if matches!(host.phase, WorkerPhase::Finished { .. }) {
            Some(RejectionReason::AlreadyFinished)
        } else if matches!(
            host.phase,
            WorkerPhase::Paused { .. } | WorkerPhase::Faulted
        ) {
            Some(RejectionReason::ResourcePaused)
        } else if request.expected_revision.get() != self.store.viewer_cursor(seat)? {
            Some(RejectionReason::StaleRevision)
        } else {
            None
        };
        if let Some(reason) = reason {
            return self.reject(seat, request, reason, host, elapsed, now);
        }
        let operation = match &request.command {
            CommandBody::Decision { token, response } => CoreOperation::Decision {
                seat: PlayerId(seat),
                token: token.clone(),
                response: response.clone(),
            },
            CommandBody::Concede => CoreOperation::Concede {
                seat: PlayerId(seat),
            },
        };
        let mut candidate = self.game.fork();
        if let Err(reason) = apply_input(&mut candidate, &operation) {
            let rejection = if reason == "query resource bound" {
                host.phase = WorkerPhase::Paused {
                    reason: PauseReason::QueryLimit,
                };
                RejectionReason::ResourcePaused
            } else {
                RejectionReason::InvalidResponse
            };
            return self.reject(seat, request, rejection, host, elapsed, now);
        }
        bind_pending(&candidate, &mut host, &operation, &request.command_id)?;
        host.phase = WorkerPhase::Running;
        host.ready = [false; 4];
        let result = SubmissionResult::Accepted {
            command_id: request.command_id.clone(),
        };
        self.commit(
            CandidateState {
                game: candidate,
                host,
                execution_updates: vec![],
            },
            Action::Core {
                operation,
                command: Some(CommandStamp::from_request(&request)),
            },
            elapsed,
            std::array::from_fn(|_| vec![]),
            Some(CommandCommit {
                seat,
                request,
                result: result.clone(),
            }),
            now,
        )?;
        Ok(result)
    }
    /// Broker-only: caller removed a known, not-yet-sent queued request. This
    /// cannot cancel/undo an accepted core input and never reserves unknown IDs.
    pub fn cancel_queued(
        &mut self,
        seat: u8,
        request: CommandEnvelope,
        now: Instant,
    ) -> Result<SubmissionResult, String> {
        let busy = Instant::now();
        let result = self.cancel_queued_inner(seat, request, now);
        if let Some(anchor) = self.last_sample {
            self.last_sample = Some(
                anchor
                    .checked_add(busy.elapsed())
                    .ok_or("clock anchor overflow")?,
            );
        }
        result
    }
    fn cancel_queued_inner(
        &mut self,
        seat: u8,
        request: CommandEnvelope,
        now: Instant,
    ) -> Result<SubmissionResult, String> {
        if self.halted {
            return Err("worker is halted".into());
        }
        if seat >= 4 {
            return Err("invalid authenticated seat".into());
        }
        self.decoder.decode(&canonical(&request)?)?;
        if let Some(result) = self.store.existing_submission(seat, &request)? {
            return Ok(result);
        }
        let elapsed = self.elapsed(now)?;
        let mut host = self.host.clone();
        debit(&mut host, elapsed);
        let result = SubmissionResult::Rejected {
            command_id: request.command_id.clone(),
            reason: RejectionReason::Cancelled,
        };
        let pubs = publications(&self.game, &self.host, &mut host, false, None)?;
        self.commit(
            CandidateState {
                game: self.game.fork(),
                host,
                execution_updates: vec![],
            },
            Action::Cancelled {
                seat,
                request: request.clone(),
            },
            elapsed,
            pubs,
            Some(CommandCommit {
                seat,
                request,
                result: result.clone(),
            }),
            now,
        )?;
        Ok(result)
    }
    pub fn advance(&mut self, now: Instant) -> Result<ExecutionStatus, String> {
        let busy = Instant::now();
        let result = self.advance_inner(now);
        let duration = busy.elapsed();
        if let Some(anchor) = self.last_sample {
            self.last_sample = Some(
                anchor
                    .checked_add(duration)
                    .ok_or("clock anchor overflow")?,
            );
        }
        result
    }
    fn advance_inner(&mut self, now: Instant) -> Result<ExecutionStatus, String> {
        if self.halted {
            return Err("worker is halted".into());
        }
        if !matches!(self.host.phase, WorkerPhase::Running) {
            return Ok(execution(&self.host));
        }
        let elapsed = self.elapsed(now)?;
        let mut host = self.host.clone();
        debit(&mut host, elapsed);
        let mut candidate = self.game.fork();
        let report = candidate.advance_with_report(Budget::new(QUANTUM));
        let expected_return = AdvanceStop::from_advance(report.advance);
        let execution_updates = process_facts(
            &mut host,
            &report.processed,
            self.head.boundary,
            candidate.rule_boundary(),
        )?;
        let (coherent, terminal) = finish_advance(&mut host, &expected_return);
        let pubs = match publications(&candidate, &self.host, &mut host, coherent, terminal) {
            Ok(pubs) => pubs,
            Err(reason)
                if reason.contains("resource bound") || reason.contains("JavaScript safe") =>
            {
                let mut paused = self.host.clone();
                debit(&mut paused, elapsed);
                paused.phase = WorkerPhase::Paused {
                    reason: if reason.starts_with("publication")
                        || reason.contains("JavaScript safe")
                    {
                        PauseReason::PublicationLimit
                    } else {
                        PauseReason::QueryLimit
                    },
                };
                let pubs = publications(&self.game, &self.host, &mut paused, false, None)?;
                self.commit(
                    CandidateState {
                        game: self.game.fork(),
                        host: paused,
                        execution_updates: vec![],
                    },
                    Action::Clock {
                        control: ClockControl::ResourcePause {
                            reason: if reason.starts_with("publication")
                                || reason.contains("JavaScript safe")
                            {
                                PauseReason::PublicationLimit
                            } else {
                                PauseReason::QueryLimit
                            },
                        },
                    },
                    elapsed,
                    pubs,
                    None,
                    now,
                )?;
                return Ok(ExecutionStatus::ResourcePaused);
            }
            Err(reason) => return Err(reason),
        };
        self.commit(
            CandidateState {
                game: candidate,
                host,
                execution_updates,
            },
            Action::Core {
                operation: CoreOperation::Advance {
                    budget: QUANTUM,
                    expected_return,
                    processed: report.processed,
                },
                command: None,
            },
            elapsed,
            pubs,
            None,
            now,
        )?;
        Ok(execution(&self.host))
    }
    pub fn clock(
        &mut self,
        control: ClockControl,
        now: Instant,
    ) -> Result<ExecutionStatus, String> {
        let busy = Instant::now();
        let result = self.clock_inner(control, now);
        let duration = busy.elapsed();
        if let Some(anchor) = self.last_sample {
            self.last_sample = Some(
                anchor
                    .checked_add(duration)
                    .ok_or("clock anchor overflow")?,
            );
        }
        result
    }
    fn clock_inner(
        &mut self,
        control: ClockControl,
        now: Instant,
    ) -> Result<ExecutionStatus, String> {
        if self.halted {
            return Err("worker is halted".into());
        }
        let elapsed = self.elapsed(now)?;
        let mut host = self.host.clone();
        debit(&mut host, elapsed);
        apply_control(&mut host, control)?;
        let pubs = publications(&self.game, &self.host, &mut host, false, None)?;
        self.commit(
            CandidateState {
                game: self.game.fork(),
                host,
                execution_updates: vec![],
            },
            Action::Clock { control },
            elapsed,
            pubs,
            None,
            now,
        )?;
        Ok(execution(&self.host))
    }
}
fn bind_pending(
    game: &Game,
    host: &mut HostState,
    operation: &CoreOperation,
    command_id: &str,
) -> Result<(), String> {
    let counts = game.accepted_input_counts()?;
    let (seat, identity) = match operation {
        CoreOperation::Decision { seat, .. } => {
            let input = game
                .journal()
                .last()
                .ok_or("accepted decision missing journal input")?;
            if input.player != *seat {
                return Err("accepted decision player mismatch".into());
            }
            (
                *seat,
                CoreInputIdentity::Decision {
                    journal_ordinal: counts
                        .decisions
                        .checked_sub(1)
                        .ok_or("missing accepted decision")?,
                    decision_id: input.decision_id,
                    player: *seat,
                },
            )
        }
        CoreOperation::Concede { seat } => (
            *seat,
            CoreInputIdentity::Concession {
                concession_ordinal: counts
                    .concessions
                    .checked_sub(1)
                    .ok_or("missing accepted concession")?,
                player: *seat,
            },
        ),
        CoreOperation::Advance { .. } => return Err("advance cannot bind a new command".into()),
    };
    if host.outstanding.len() >= 4
        || host
            .outstanding
            .iter()
            .any(|p| p.identity == identity || (p.seat == seat.0 && p.command_id == command_id))
    {
        return Err("outstanding command identity bound".into());
    }
    host.outstanding.push(PendingCommand {
        seat: seat.0,
        command_id: command_id.into(),
        identity,
    });
    Ok(())
}
fn process_facts(
    host: &mut HostState,
    facts: &[InputProcessingFact],
    before: u64,
    after: u64,
) -> Result<Vec<ExecutionUpdate>, String> {
    if facts.len() > 4 {
        return Err("input processing fact bound".into());
    }
    let mut previous = before;
    let mut updates = Vec::with_capacity(facts.len());
    for fact in facts {
        if fact.boundary <= before || fact.boundary < previous || fact.boundary > after {
            return Err("input processing boundary mismatch".into());
        }
        previous = fact.boundary;
        let index = host
            .outstanding
            .iter()
            .position(|p| p.identity == fact.identity)
            .ok_or("consumption fact lacks outstanding accepted command")?;
        let pending = host.outstanding.remove(index);
        let status = match (&fact.identity, &fact.cause) {
            (CoreInputIdentity::Decision { .. }, InputProcessingCause::RulesStep)
            | (CoreInputIdentity::Concession { .. }, InputProcessingCause::DepartureStep) => {
                CommandExecutionStatus::ConsumedByRules
            }
            (CoreInputIdentity::Decision { .. }, InputProcessingCause::DepartureStep) => {
                CommandExecutionStatus::DiscardedByDeparture
            }
            _ => return Err("invalid concession processing cause".into()),
        };
        updates.push(ExecutionUpdate {
            seat: pending.seat,
            command_id: pending.command_id,
            status,
        });
    }
    Ok(updates)
}
fn apply_input(game: &mut Game, operation: &CoreOperation) -> Result<(), String> {
    match operation {
        CoreOperation::Decision {
            seat,
            token,
            response,
        } => {
            let result = query(
                game,
                game.check_player_response(
                    *seat,
                    token.clone(),
                    response.clone(),
                    Budget::new(QUANTUM),
                ),
            )?;
            let QueryValue::Validated(receipt) = result else {
                return Err("validation returned wrong query value".into());
            };
            game.submit(receipt)
        }
        CoreOperation::Concede { seat } => game.concede(*seat),
        CoreOperation::Advance { .. } => Err("not a core input".into()),
    }
}
fn apply_control(host: &mut HostState, control: ClockControl) -> Result<(), String> {
    if matches!(
        host.phase,
        WorkerPhase::Faulted | WorkerPhase::Finished { .. }
    ) {
        return Err("terminal/faulted worker cannot change clock state".into());
    }
    match control {
        ClockControl::Tick => {}
        ClockControl::Pause => {
            host.phase = WorkerPhase::Paused {
                reason: PauseReason::Operator,
            }
        }
        ClockControl::Resume => {
            if !matches!(host.phase, WorkerPhase::Paused { .. }) {
                return Err("worker is not paused".into());
            }
            if host
                .ready
                .iter()
                .zip(host.remaining_ns)
                .any(|(ready, left)| *ready && left == 0)
            {
                return Err("exhausted clock cannot be refilled by resume".into());
            }
            host.phase = if host.ready.iter().any(|ready| *ready) {
                WorkerPhase::Waiting
            } else {
                WorkerPhase::Running
            };
        }
        ClockControl::ResourcePause { reason } => {
            if !matches!(
                reason,
                PauseReason::QueryLimit | PauseReason::PublicationLimit
            ) {
                return Err("invalid resource pause cause".into());
            }
            host.phase = WorkerPhase::Paused { reason };
        }
    }
    Ok(())
}
struct ReplayCursor {
    game: Game,
    host: HostState,
    expected_head: Head,
}
impl ReplayCursor {
    fn stage_record(
        &mut self,
        store: &MatchStore,
        decoder: &crate::protocol::CommandDecoder,
        durable: DurableRecord,
    ) -> Result<(AfterHashJob, Option<(u8, String)>), String> {
        // Checkpoint0's actual digest is proven before this private cursor is
        // built. Expected heads may be unproved WITHIN a six-record batch;
        // every exact candidate snapshot is hashed and matched at its barrier.
        // Such cursor progress never installs a Worker or publishes anything.
        if durable.head.sequence
            != self
                .expected_head
                .sequence
                .checked_add(1)
                .ok_or("record sequence overflow")?
            || durable.before_boundary != self.expected_head.boundary
            || durable.before_state_sha256 != self.expected_head.state_sha256
            || self.game.rule_boundary() != durable.before_boundary
        {
            return Err("operation before-boundary/hash/sequence mismatch".into());
        }
        let record: WorkerRecord = strict_read(&canonical(&durable.record)?)?;
        if record.worker_record_version != VERSION
            || record.before_host_sha256 != digest(&self.host)?
        {
            return Err("worker record version/host mismatch".into());
        }
        let before = self.host.clone();
        let mut host = before.clone();
        debit(&mut host, record.elapsed_ns);
        let mut candidate = self.game.fork();
        let mut coherent = false;
        let mut terminal = None;
        let mut expected_command = None;
        let mut execution_updates = vec![];
        match &record.action {
            Action::Core { operation, command } => {
                if matches!(
                    host.phase,
                    WorkerPhase::Paused { .. }
                        | WorkerPhase::Faulted
                        | WorkerPhase::Finished { .. }
                ) {
                    return Err("record applies core work while stopped".into());
                }
                match operation {
                    CoreOperation::Advance {
                        budget,
                        expected_return,
                        processed,
                    } => {
                        if *budget != QUANTUM
                            || command.is_some()
                            || !matches!(host.phase, WorkerPhase::Running)
                        {
                            return Err("invalid automatic operation contract".into());
                        }
                        let report = candidate.advance_with_report(Budget::new(*budget));
                        let actual = AdvanceStop::from_advance(report.advance);
                        if actual != *expected_return || report.processed != *processed {
                            return Err("recorded advance return/consumption facts differ".into());
                        }
                        execution_updates = process_facts(
                            &mut host,
                            &report.processed,
                            self.expected_head.boundary,
                            candidate.rule_boundary(),
                        )?;
                        (coherent, terminal) = finish_advance(&mut host, &actual);
                    }
                    CoreOperation::Decision { .. } | CoreOperation::Concede { .. } => {
                        let (seat, request) = command
                            .as_ref()
                            .ok_or("accepted operation lacks command metadata")?
                            .request(operation)?;
                        decoder.decode(&canonical(&request)?)?;
                        if seat >= 4
                            || request.expected_revision.get()
                                != before.viewer_cursors[usize::from(seat)]
                        {
                            return Err("accepted record uses wrong viewer revision".into());
                        }
                        apply_input(&mut candidate, operation)?;
                        bind_pending(&candidate, &mut host, operation, &request.command_id)?;
                        host.phase = WorkerPhase::Running;
                        host.ready = [false; 4];
                        expected_command = Some((
                            seat,
                            request.clone(),
                            SubmissionResult::Accepted {
                                command_id: request.command_id.clone(),
                            },
                        ));
                    }
                }
            }
            Action::Rejected {
                seat,
                request,
                reason,
            } => {
                if *seat >= 4 {
                    return Err("rejected record has invalid seat".into());
                }
                decoder.decode(&canonical(request)?)?;
                let expected = if matches!(host.phase, WorkerPhase::Finished { .. }) {
                    RejectionReason::AlreadyFinished
                } else if matches!(
                    host.phase,
                    WorkerPhase::Paused { .. } | WorkerPhase::Faulted
                ) {
                    RejectionReason::ResourcePaused
                } else if request.expected_revision.get()
                    != before.viewer_cursors[usize::from(*seat)]
                {
                    RejectionReason::StaleRevision
                } else {
                    let operation = match &request.command {
                        CommandBody::Decision { token, response } => CoreOperation::Decision {
                            seat: PlayerId(*seat),
                            token: token.clone(),
                            response: response.clone(),
                        },
                        CommandBody::Concede => CoreOperation::Concede {
                            seat: PlayerId(*seat),
                        },
                    };
                    match apply_input(&mut candidate, &operation) {
                        Ok(()) => return Err("rejected record contains a valid core input".into()),
                        Err(reason) if reason == "query resource bound" => {
                            host.phase = WorkerPhase::Paused {
                                reason: PauseReason::QueryLimit,
                            };
                            RejectionReason::ResourcePaused
                        }
                        Err(_) => RejectionReason::InvalidResponse,
                    }
                };
                if expected != *reason {
                    return Err("durable rejection reason differs".into());
                }
                candidate = self.game.fork();
                expected_command = Some((
                    *seat,
                    request.clone(),
                    SubmissionResult::Rejected {
                        command_id: request.command_id.clone(),
                        reason: *reason,
                    },
                ));
            }
            Action::Cancelled { seat, request } => {
                if *seat >= 4 {
                    return Err("cancel record has invalid seat".into());
                }
                decoder.decode(&canonical(request)?)?;
                // No core validation, input conversion, submission or advance.
                expected_command = Some((
                    *seat,
                    request.clone(),
                    SubmissionResult::Rejected {
                        command_id: request.command_id.clone(),
                        reason: RejectionReason::Cancelled,
                    },
                ));
            }
            Action::Clock { control } => apply_control(&mut host, *control)?,
        }
        let pubs = publications(&candidate, &before, &mut host, coherent, terminal)?;
        if execution_updates != record.execution_updates
            || host != record.after_host
            || candidate.rule_boundary() != durable.head.boundary
        {
            return Err("operation after-state/host/hash mismatch".into());
        }
        for seat in 0..4u8 {
            let index = usize::from(seat);
            let hashes = pubs[index]
                .iter()
                .map(digest)
                .collect::<Result<Vec<_>, _>>()?;
            if hashes != record.publication_hashes[index] {
                return Err("private publication hash mismatch".into());
            }
            if !pubs[index].is_empty() {
                let page = store.stream_page(
                    seat,
                    before.viewer_cursors[index],
                    u32::try_from(pubs[index].len()).map_err(|_| "publication count overflow")?,
                )?;
                if page.entries.len() != pubs[index].len() {
                    return Err("private outbox lost committed publication".into());
                }
                for (offset, (actual, expected)) in
                    page.entries.iter().zip(&pubs[index]).enumerate()
                {
                    let cursor = before.viewer_cursors[index]
                        .checked_add(offset as u64 + 1)
                        .ok_or("publication cursor overflow")?;
                    if actual.cursor.get() != cursor || actual.publication != *expected {
                        return Err("private outbox content/cursor mismatch".into());
                    }
                }
            }
        }
        for update in &execution_updates {
            let expected = crate::protocol::CommandExecution {
                command_id: update.command_id.clone(),
                status: update.status.clone(),
            };
            if store.execution(update.seat, &update.command_id)? != Some(expected) {
                return Err("command execution disposition differs from committed facts".into());
            }
        }
        if let Some((seat, request, SubmissionResult::Rejected { reason, .. })) = &expected_command
        {
            let status = if *reason == RejectionReason::Cancelled {
                CommandExecutionStatus::Cancelled
            } else {
                CommandExecutionStatus::Rejected { reason: *reason }
            };
            if store.execution(*seat, &request.command_id)?
                != Some(crate::protocol::CommandExecution {
                    command_id: request.command_id.clone(),
                    status,
                })
            {
                return Err("rejected command execution status differs".into());
            }
        }
        let command_key = expected_command
            .as_ref()
            .map(|(seat, request, _)| (*seat, request.command_id.clone()));
        if let Some((seat, request, result)) = expected_command
            && store.existing_submission(seat, &request)? != Some(result)
        {
            return Err("durable command outcome is missing or inconsistent".into());
        }
        // Retain an exact AFTER snapshot for EVERY record, including Clock,
        // Rejected and Cancelled operations whose State is unchanged. Its
        // actual digest is mandatory before this cursor can become a Worker.
        let job = AfterHashJob {
            expected: durable.head.clone(),
            game: candidate.fork(),
        };
        self.game = candidate;
        self.host = host;
        self.expected_head = durable.head;
        Ok((job, command_key))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
        time::Duration,
    };
    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            let p = std::env::temp_dir().join(format!(
                "mtg-worker-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&p).unwrap();
            Self(p)
        }
        fn pod(&self) -> PathBuf {
            self.0.join("pod")
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn catalog() -> Catalog {
        Catalog::read(include_str!(
            "../../game-engine/tests/fixtures/expected-match-ir.json"
        ))
        .unwrap()
    }
    fn create() -> (Temp, Worker) {
        let temp = Temp::new();
        let worker = Worker::create(
            &temp.pod(),
            catalog(),
            MatchConfig::foundations([3; 32], PlayerId(0)),
            "a1",
            Instant::now(),
        )
        .unwrap();
        (temp, worker)
    }
    fn now(worker: &Worker) -> Instant {
        worker.last_sample.unwrap()
    }
    fn settle(worker: &mut Worker) {
        for _ in 0..1000 {
            if !matches!(worker.host.phase, WorkerPhase::Running) {
                return;
            }
            worker.advance(now(worker)).unwrap();
        }
        panic!("worker progress limit");
    }
    fn decision(worker: &Worker) -> (u8, mtg_core::PlayerRequest) {
        for seat in 0..4u8 {
            if let Some(request) = observe(&worker.game, seat).unwrap().decision {
                return (seat, request);
            }
        }
        panic!("no ready request")
    }
    fn request(worker: &Worker, seat: u8, id: u64, body: CommandBody) -> CommandEnvelope {
        CommandEnvelope {
            protocol_version: 1,
            command_id: format!("{id:032x}"),
            expected_revision: Counter::new(worker.host.viewer_cursors[usize::from(seat)]),
            command: body,
        }
    }
    fn keep(worker: &Worker, id: u64) -> (u8, CommandEnvelope) {
        let (seat, r) = decision(worker);
        assert!(matches!(r, mtg_core::PlayerRequest::Mulligan { .. }));
        (
            seat,
            request(
                worker,
                seat,
                id,
                CommandBody::Decision {
                    token: r.token().clone(),
                    response: PlayerResponse::Mulligan { take: false },
                },
            ),
        )
    }
    fn mutate_record(directory: &Path, mutate: impl FnOnce(&mut serde_json::Value)) {
        let db = rusqlite::Connection::open(directory.join("match.sqlite3")).unwrap();
        let body: String = db
            .query_row("SELECT body FROM records WHERE sequence=1", [], |row| {
                row.get(0)
            })
            .unwrap();
        let mut value: serde_json::Value = serde_json::from_str(&body).unwrap();
        mutate(&mut value);
        db.execute(
            "UPDATE records SET body=?1 WHERE sequence=1",
            [canonical(&value).unwrap()],
        )
        .unwrap();
    }
    #[test]
    fn pending_acceptance_is_durable_before_progress_and_retry_precedes_staleness() {
        let (temp, mut worker) = create();
        settle(&mut worker);
        let (seat, input) = keep(&worker, 1);
        let before_boundary = worker.head.boundary;
        let cursor = worker.host.viewer_cursors;
        let accepted = worker.submit(seat, input.clone(), now(&worker)).unwrap();
        assert!(matches!(accepted, SubmissionResult::Accepted { .. }));
        assert_eq!(worker.head.boundary, before_boundary);
        assert_eq!(worker.host.viewer_cursors, cursor);
        assert_eq!(worker.game.journal().len(), 1);
        let digest = worker.state_sha256().unwrap();
        let head = worker.head.clone();
        drop(worker);
        let mut recovered = Worker::recover(&temp.pod(), catalog(), "a1", Instant::now()).unwrap();
        assert_eq!(recovered.head, head);
        assert_eq!(recovered.state_sha256().unwrap(), digest);
        assert_eq!(recovered.game.journal().len(), 1);
        let mut reference = recovered.game.fork();
        let expected = AdvanceStop::from_advance(reference.advance(Budget::new(QUANTUM)));
        recovered.advance(now(&recovered)).unwrap();
        assert_eq!(
            recovered.state_sha256().unwrap(),
            reference.state_digest().unwrap()
        );
        assert!(matches!(
            expected,
            AdvanceStop::Awaiting { .. } | AdvanceStop::Suspended
        ));
        let after = recovered.head.clone();
        assert_eq!(
            recovered
                .submit(seat, input.clone(), now(&recovered))
                .unwrap(),
            accepted
        );
        assert_eq!(recovered.head, after);
        assert_eq!(recovered.game.journal().len(), 1);
        let mut conflict = input;
        conflict.command = CommandBody::Concede;
        assert!(recovered.submit(seat, conflict, now(&recovered)).is_err());
    }
    #[test]
    fn exact_mid_shuffle_boundary_and_rng_restore_without_auto_drain() {
        let (temp, mut worker) = create();
        worker.advance(now(&worker)).unwrap();
        assert!(matches!(worker.host.phase, WorkerPhase::Running));
        assert_eq!(worker.head.boundary, QUANTUM);
        let before = worker.head.clone();
        let mut reference = worker.game.fork();
        reference.advance(Budget::new(QUANTUM));
        drop(worker);
        let mut recovered = Worker::recover(&temp.pod(), catalog(), "a1", Instant::now()).unwrap();
        assert_eq!(recovered.head, before);
        recovered.advance(now(&recovered)).unwrap();
        assert_eq!(
            recovered.state_sha256().unwrap(),
            reference.state_digest().unwrap()
        );
        assert_eq!(recovered.host.remaining_ns, [INITIAL_CLOCK_NS; 4]);
    }
    #[test]
    fn clock_debits_only_offered_ready_seat_and_expiry_is_not_a_magic_outcome() {
        let (temp, mut worker) = create();
        settle(&mut worker);
        let (seat, _) = decision(&worker);
        let index = usize::from(seat);
        let before_sha = worker.state_sha256().unwrap();
        let before_cursor = worker.host.viewer_cursors;
        worker
            .clock(ClockControl::Tick, now(&worker) + Duration::from_secs(10))
            .unwrap();
        for other in 0..4 {
            assert_eq!(
                worker.host.remaining_ns[other],
                INITIAL_CLOCK_NS - if other == index { 10_000_000_000 } else { 0 }
            );
        }
        assert_eq!(worker.host.viewer_cursors, before_cursor);
        assert_eq!(worker.state_sha256().unwrap(), before_sha);
        worker.clock(ClockControl::Pause, now(&worker)).unwrap();
        let left = worker.host.remaining_ns;
        worker
            .clock(ClockControl::Tick, now(&worker) + Duration::from_secs(3600))
            .unwrap();
        assert_eq!(worker.host.remaining_ns, left);
        worker.clock(ClockControl::Resume, now(&worker)).unwrap();
        worker
            .clock(ClockControl::Tick, now(&worker) + Duration::from_secs(1800))
            .unwrap();
        assert_eq!(worker.host.remaining_ns[index], 0);
        assert!(matches!(
            worker.host.phase,
            WorkerPhase::Paused {
                reason: PauseReason::ClockExpired
            }
        ));
        assert_eq!(worker.state_sha256().unwrap(), before_sha);
        for seat in 0..4u8 {
            assert!(
                worker
                    .stream_page(seat, 0, 32)
                    .unwrap()
                    .entries
                    .iter()
                    .all(|entry| !matches!(entry.publication, Publication::Outcome { .. }))
            );
        }
        let state = worker.host.clone();
        drop(worker);
        let mut recovered = Worker::recover(&temp.pod(), catalog(), "a1", Instant::now()).unwrap();
        assert_eq!(recovered.host, state);
        assert!(
            recovered
                .clock(ClockControl::Resume, now(&recovered))
                .is_err()
        );
    }
    #[test]
    fn permanent_stale_rejection_is_idempotent_and_does_not_append_core_input() {
        let (_temp, mut worker) = create();
        settle(&mut worker);
        let (seat, mut stale) = keep(&worker, 2);
        stale.expected_revision = Counter::new(0);
        let result = worker.submit(seat, stale.clone(), now(&worker)).unwrap();
        assert!(matches!(
            result,
            SubmissionResult::Rejected {
                reason: RejectionReason::StaleRevision,
                ..
            }
        ));
        assert!(worker.game.journal().is_empty());
        let (seat, valid) = keep(&worker, 3);
        worker.submit(seat, valid, now(&worker)).unwrap();
        settle(&mut worker);
        let head = worker.head.clone();
        assert_eq!(worker.submit(seat, stale, now(&worker)).unwrap(), result);
        assert_eq!(worker.head, head);
        assert_eq!(worker.game.journal().len(), 1);
    }
    fn replay_history_fixture() -> (Temp, Worker, Game, HostState, Head) {
        let (temp, mut worker) = create();
        let initial_game = worker.game.fork();
        let initial_host = worker.host.clone();
        let initial_head = worker.head.clone();
        settle(&mut worker);
        for id in 1..=4 {
            let (seat, input) = keep(&worker, id);
            worker.submit(seat, input, now(&worker)).unwrap();
            settle(&mut worker);
        }
        worker.clock(ClockControl::Pause, now(&worker)).unwrap();
        worker
            .clock(ClockControl::Tick, now(&worker) + Duration::from_secs(3))
            .unwrap();
        worker.clock(ClockControl::Resume, now(&worker)).unwrap();
        let input = request(&worker, 3, 90, CommandBody::Concede);
        worker.submit(3, input, now(&worker)).unwrap();
        settle(&mut worker);
        (temp, worker, initial_game, initial_host, initial_head)
    }
    #[test]
    fn recovery_parallel_hash_chain_matches_old_before_oracle_for_every_record() {
        let (temp, worker, initial_game, initial_host, initial_head) = replay_history_fixture();
        let expected_head = worker.head.clone();
        let expected_host = worker.host.clone();
        let expected_sha = worker.game.state_digest().unwrap();
        let records = worker.store.records_after(0, 128).unwrap();
        assert!(records.len() > 12);
        let mut reference = vec![];
        for serial_oracle in [true, false] {
            let mut cursor = ReplayCursor {
                game: initial_game.fork(),
                host: initial_host.clone(),
                expected_head: initial_head.clone(),
            };
            let mut pool = AfterHashPool::new().unwrap();
            for (index, record) in records.iter().enumerate() {
                if serial_oracle {
                    // The old implementation hashed both sides of EVERY record.
                    assert_eq!(
                        cursor.game.state_digest().unwrap(),
                        record.before_state_sha256
                    );
                }
                let (job, _) = cursor
                    .stage_record(&worker.store, &worker.decoder, record.clone())
                    .unwrap();
                if serial_oracle {
                    assert_eq!(job.game.state_digest().unwrap(), job.expected.state_sha256);
                    assert_eq!(job.game.rule_boundary(), job.expected.boundary);
                } else {
                    pool.enqueue(job).unwrap();
                    if (index + 1) % BATCH_SIZE == 0 {
                        pool.barrier().unwrap();
                    }
                }
                let actual = (
                    cursor.expected_head.clone(),
                    cursor.game.state_digest().unwrap(),
                    digest(&cursor.host).unwrap(),
                    cursor.game.replay_journal(),
                );
                if serial_oracle {
                    reference.push(actual);
                } else {
                    assert_eq!(actual, reference[index]);
                }
            }
            pool.finish().unwrap();
            assert_eq!(cursor.expected_head, expected_head);
            assert_eq!(cursor.host, expected_host);
            assert_eq!(cursor.game.state_digest().unwrap(), expected_sha);
        }
        drop(worker);
        let recovered = Worker::recover(&temp.pod(), catalog(), "a1", Instant::now()).unwrap();
        assert_eq!(recovered.head, expected_head);
        assert_eq!(recovered.host, expected_host);
        assert_eq!(recovered.game.state_digest().unwrap(), expected_sha);
    }
    #[test]
    fn recovery_parallel_hash_failures_never_install_into_live_worker() {
        let (_temp, worker, initial_game, initial_host, initial_head) = replay_history_fixture();
        let records = worker.store.records_after(0, 128).unwrap();
        let middle = records.len() / 2;
        let live_head = worker.head.clone();
        let live_host = worker.host.clone();
        let live_sha = worker.game.state_digest().unwrap();
        let live_journal = worker.game.replay_journal();
        for variant in 0..3 {
            let mut cursor = ReplayCursor {
                game: initial_game.fork(),
                host: initial_host.clone(),
                expected_head: initial_head.clone(),
            };
            let mut pool = AfterHashPool::new().unwrap();
            for record in &records[..middle] {
                let (job, _) = cursor
                    .stage_record(&worker.store, &worker.decoder, record.clone())
                    .unwrap();
                pool.enqueue(job).unwrap();
                pool.barrier().unwrap();
            }
            let mut altered = records[middle].clone();
            if variant == 0 {
                altered.before_state_sha256 = "0".repeat(64);
            } else {
                altered.head.state_sha256 = "0".repeat(64);
            }
            if variant == 0 {
                assert!(
                    cursor
                        .stage_record(&worker.store, &worker.decoder, altered)
                        .is_err()
                );
            } else {
                let (job, _) = cursor
                    .stage_record(&worker.store, &worker.decoder, altered.clone())
                    .unwrap();
                pool.enqueue(job).unwrap();
                if variant == 2 {
                    // Even an internally connected forged successor cannot
                    // turn the first failed actual digest into a verified head.
                    let mut subsequent = records[middle + 1].clone();
                    subsequent.before_state_sha256 = altered.head.state_sha256;
                    let (job, _) = cursor
                        .stage_record(&worker.store, &worker.decoder, subsequent)
                        .unwrap();
                    pool.enqueue(job).unwrap();
                }
                assert!(pool.barrier().is_err());
            }
            assert_eq!(worker.head, live_head);
            assert_eq!(worker.host, live_host);
            assert_eq!(worker.game.state_digest().unwrap(), live_sha);
            assert_eq!(worker.game.replay_journal(), live_journal);
        }
    }
    #[test]
    fn recovery_rejects_a_rehashed_two_record_middle_chain() {
        let (temp, worker, _, _, _) = replay_history_fixture();
        let mut records = worker.store.records_after(0, 128).unwrap();
        let middle = records.len() / 2;
        records[middle].head.state_sha256 = "0".repeat(64);
        records[middle + 1].before_state_sha256 = records[middle].head.state_sha256.clone();
        assert_eq!(
            records[middle + 1].before_state_sha256,
            records[middle].head.state_sha256
        );
        drop(worker);
        let db = rusqlite::Connection::open(temp.pod().join("match.sqlite3")).unwrap();
        for record in &records[middle..=middle + 1] {
            db.execute(
                "UPDATE records SET body=?1 WHERE sequence=?2",
                rusqlite::params![
                    canonical(record).unwrap(),
                    i64::try_from(record.head.sequence).unwrap()
                ],
            )
            .unwrap();
        }
        drop(db);
        assert!(Worker::recover(&temp.pod(), catalog(), "a1", Instant::now()).is_err());
    }
    #[test]
    fn exact_recovery_rejects_version_operation_hash_host_and_checkpoint_corruption() {
        for mutate in [
            (|v: &mut serde_json::Value| {
                v["record"]["worker_record_version"] = (VERSION + 1).into()
            }) as fn(&mut serde_json::Value),
            |v| v["record"]["action"]["operation"]["budget"] = 65.into(),
            |v| v["before_state_sha256"] = "0".repeat(64).into(),
            |v| v["record"]["after_host"]["remaining_ns"][0] = 1.into(),
            |v| v["record"]["extra_private_override"] = true.into(),
        ] {
            let (temp, mut worker) = create();
            worker.advance(now(&worker)).unwrap();
            drop(worker);
            mutate_record(&temp.pod(), mutate);
            assert!(Worker::recover(&temp.pod(), catalog(), "a1", Instant::now()).is_err());
        }
        let (temp, worker) = create();
        drop(worker);
        let mut file = OpenOptions::new()
            .append(true)
            .open(temp.pod().join("checkpoint0.json"))
            .unwrap();
        file.write_all(b" ").unwrap();
        drop(file);
        assert!(Worker::recover(&temp.pod(), catalog(), "a1", Instant::now()).is_err());
    }
    #[test]
    fn failed_transaction_never_publishes_candidate_or_acknowledges_input() {
        let (temp, mut worker) = create();
        settle(&mut worker);
        let (seat, input) = keep(&worker, 4);
        let head = worker.head.clone();
        let cursors = worker.host.viewer_cursors;
        let db = rusqlite::Connection::open(temp.pod().join("match.sqlite3")).unwrap();
        db.execute_batch("CREATE TRIGGER fail_record BEFORE INSERT ON records BEGIN SELECT RAISE(ABORT,'injected'); END;").unwrap();
        assert!(worker.submit(seat, input.clone(), now(&worker)).is_err());
        assert!(worker.halted);
        assert_eq!(worker.head, head);
        assert_eq!(worker.state_sha256().unwrap(), head.state_sha256);
        assert!(worker.game.journal().is_empty());
        assert_eq!(worker.host.viewer_cursors, cursors);
        db.execute_batch("DROP TRIGGER fail_record;").unwrap();
        drop(db);
        drop(worker);
        let mut recovered = Worker::recover(&temp.pod(), catalog(), "a1", Instant::now()).unwrap();
        assert!(matches!(
            recovered.submit(seat, input, now(&recovered)).unwrap(),
            SubmissionResult::Accepted { .. }
        ));
    }
    #[test]
    fn actual_concession_terminal_result_publishes_once_to_every_seat() {
        let (_temp, mut worker) = create();
        settle(&mut worker);
        for seat in 1..4u8 {
            let input = request(&worker, seat, 10 + u64::from(seat), CommandBody::Concede);
            assert!(matches!(
                worker.submit(seat, input, now(&worker)).unwrap(),
                SubmissionResult::Accepted { .. }
            ));
        }
        settle(&mut worker);
        assert_eq!(
            worker.host.phase,
            WorkerPhase::Finished {
                result: RulesResult::Win {
                    player: PlayerId(0)
                }
            }
        );
        for seat in 0..4u8 {
            let page = worker.stream_page(seat, 0, 32).unwrap();
            assert_eq!(
                page.entries
                    .iter()
                    .filter(|entry| matches!(
                        entry.publication,
                        Publication::Outcome {
                            result: RulesResult::Win {
                                player: PlayerId(0)
                            }
                        }
                    ))
                    .count(),
                1
            );
        }
        let head = worker.head.clone();
        assert_eq!(
            worker.advance(now(&worker)).unwrap(),
            ExecutionStatus::Finished
        );
        assert_eq!(worker.head, head);
    }
    #[test]
    fn recovery_compares_actual_private_outbox_not_only_recorded_state() {
        let (temp, mut worker) = create();
        settle(&mut worker);
        drop(worker);
        let db = rusqlite::Connection::open(temp.pod().join("match.sqlite3")).unwrap();
        let body: String = db
            .query_row(
                "SELECT body FROM outbox WHERE seat=1 AND cursor=1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let mut value: serde_json::Value = serde_json::from_str(&body).unwrap();
        value["entries"][0]["publication"] =
            serde_json::json!({"kind":"lifecycle","status":{"status":"running"}});
        db.execute(
            "UPDATE outbox SET body=?1 WHERE seat=1 AND cursor=1",
            [canonical(&value).unwrap()],
        )
        .unwrap();
        drop(db);
        assert!(Worker::recover(&temp.pod(), catalog(), "a1", Instant::now()).is_err());
    }
    #[test]
    fn queued_cancel_is_durable_immutable_and_never_applies_core_input() {
        let (temp, mut worker) = create();
        settle(&mut worker);
        let (seat, mut input) = keep(&worker, 50);
        // Cancellation is host-only and precedes viewer freshness/core validation.
        input.expected_revision = Counter::new(0);
        let before = worker.head.clone();
        let journal = worker.game.journal().to_vec();
        let cursor = worker.host.viewer_cursors;
        let result = worker
            .cancel_queued(seat, input.clone(), now(&worker) + Duration::from_secs(7))
            .unwrap();
        assert_eq!(
            result,
            SubmissionResult::Rejected {
                command_id: input.command_id.clone(),
                reason: RejectionReason::Cancelled,
            }
        );
        assert_eq!(worker.head.boundary, before.boundary);
        assert_eq!(worker.head.state_sha256, before.state_sha256);
        assert_eq!(worker.game.journal(), journal);
        assert_eq!(worker.host.viewer_cursors, cursor);
        for other in 0..4 {
            assert_eq!(
                worker.host.remaining_ns[other],
                INITIAL_CLOCK_NS
                    - if other == usize::from(seat) {
                        7_000_000_000
                    } else {
                        0
                    }
            );
        }
        let host = worker.host.clone();
        let head = worker.head.clone();
        drop(worker);
        let mut recovered = Worker::recover(&temp.pod(), catalog(), "a1", Instant::now()).unwrap();
        assert_eq!(recovered.host, host);
        assert_eq!(recovered.head, head);
        assert_eq!(
            recovered
                .submit(seat, input.clone(), now(&recovered))
                .unwrap(),
            result
        );
        assert_eq!(
            recovered
                .cancel_queued(seat, input.clone(), now(&recovered))
                .unwrap(),
            result
        );
        assert_eq!(recovered.head, head);
        assert_eq!(recovered.game.journal(), journal);
        let mut conflicting = input;
        conflicting.command = CommandBody::Concede;
        assert!(
            recovered
                .cancel_queued(seat, conflicting.clone(), now(&recovered))
                .is_err()
        );
        assert!(
            recovered
                .submit(seat, conflicting, now(&recovered))
                .is_err()
        );
        assert_eq!(recovered.head, head);
    }
    #[test]
    fn cancellation_cannot_undo_durably_accepted_pending_or_applied_input() {
        let (_temp, mut worker) = create();
        settle(&mut worker);
        let (seat, input) = keep(&worker, 51);
        let accepted = worker.submit(seat, input.clone(), now(&worker)).unwrap();
        assert!(matches!(accepted, SubmissionResult::Accepted { .. }));
        let pending_head = worker.head.clone();
        assert_eq!(
            worker
                .cancel_queued(seat, input.clone(), now(&worker))
                .unwrap(),
            accepted
        );
        assert_eq!(worker.head, pending_head);
        assert_eq!(worker.game.journal().len(), 1);
        settle(&mut worker);
        assert!(worker.head.boundary > pending_head.boundary);
        let applied_head = worker.head.clone();
        assert_eq!(
            worker.cancel_queued(seat, input, now(&worker)).unwrap(),
            accepted
        );
        assert_eq!(worker.head, applied_head);
        assert_eq!(worker.game.journal().len(), 1);
    }
    #[test]
    fn recovery_requires_explicit_cancellation_operation() {
        let (temp, mut worker) = create();
        settle(&mut worker);
        let (seat, input) = keep(&worker, 52);
        worker.cancel_queued(seat, input, now(&worker)).unwrap();
        drop(worker);
        let db = rusqlite::Connection::open(temp.pod().join("match.sqlite3")).unwrap();
        let (sequence, body): (i64, String) = db
            .query_row(
                "SELECT sequence,body FROM records ORDER BY sequence DESC LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        let mut value: serde_json::Value = serde_json::from_str(&body).unwrap();
        value["record"]["action"]["action"] = "rejected".into();
        value["record"]["action"]["reason"] = "cancelled".into();
        db.execute(
            "UPDATE records SET body=?1 WHERE sequence=?2",
            rusqlite::params![canonical(&value).unwrap(), sequence],
        )
        .unwrap();
        drop(db);
        assert!(Worker::recover(&temp.pod(), catalog(), "a1", Instant::now()).is_err());
    }
    #[test]
    fn recovery_rejects_unreferenced_command_rows() {
        let (temp, mut worker) = create();
        settle(&mut worker);
        let (seat, input) = keep(&worker, 53);
        let result = SubmissionResult::Rejected {
            command_id: input.command_id.clone(),
            reason: RejectionReason::Cancelled,
        };
        drop(worker);
        let db = rusqlite::Connection::open(temp.pod().join("match.sqlite3")).unwrap();
        db.execute(
            "INSERT INTO commands VALUES(?1,?2,?3,?4)",
            rusqlite::params![
                seat,
                input.command_id,
                canonical(&input).unwrap(),
                canonical(&result).unwrap()
            ],
        )
        .unwrap();
        drop(db);
        assert!(Worker::recover(&temp.pod(), catalog(), "a1", Instant::now()).is_err());
    }
    fn command_status(
        worker: &Worker,
        seat: u8,
        input: &CommandEnvelope,
    ) -> CommandExecutionStatus {
        worker
            .command_execution(seat, &input.command_id)
            .unwrap()
            .unwrap()
            .status
    }
    #[test]
    fn execution_stays_pending_until_exact_consumption_and_survives_recovery() {
        let (temp, mut worker) = create();
        settle(&mut worker);
        assert_eq!(execution(&worker.host), ExecutionStatus::Waiting);
        let (seat, input) = keep(&worker, 60);
        worker.submit(seat, input.clone(), now(&worker)).unwrap();
        assert_eq!(
            command_status(&worker, seat, &input),
            CommandExecutionStatus::Pending
        );
        assert_eq!(worker.host.outstanding.len(), 1);
        let pending = worker.host.outstanding[0].clone();
        assert!(
            matches!(pending.identity, CoreInputIdentity::Decision {journal_ordinal:0,player,..} if player == PlayerId(seat))
        );
        drop(worker);
        let mut recovered = Worker::recover(&temp.pod(), catalog(), "a1", Instant::now()).unwrap();
        assert_eq!(
            command_status(&recovered, seat, &input),
            CommandExecutionStatus::Pending
        );
        assert_eq!(recovered.host.outstanding, [pending]);
        let mut reference = recovered.game.fork();
        reference.advance(Budget::new(QUANTUM));
        recovered.advance(now(&recovered)).unwrap();
        assert_eq!(
            recovered.state_sha256().unwrap(),
            reference.state_digest().unwrap()
        );
        assert_eq!(
            command_status(&recovered, seat, &input),
            CommandExecutionStatus::ConsumedByRules
        );
        assert!(recovered.host.outstanding.is_empty());
        let head = recovered.head.clone();
        assert!(matches!(
            recovered
                .submit(seat, input.clone(), now(&recovered))
                .unwrap(),
            SubmissionResult::Accepted { .. }
        ));
        assert_eq!(recovered.head, head);
        drop(recovered);
        let recovered = Worker::recover(&temp.pod(), catalog(), "a1", Instant::now()).unwrap();
        assert_eq!(
            command_status(&recovered, seat, &input),
            CommandExecutionStatus::ConsumedByRules
        );
        assert!(recovered.host.outstanding.is_empty());
        assert!(
            recovered
                .command_execution((seat + 1) % 4, &input.command_id)
                .unwrap()
                .is_none()
        );
    }
    #[test]
    fn departure_discards_pending_decision_but_consumes_the_concession() {
        let (temp, mut worker) = create();
        settle(&mut worker);
        let (seat, input) = keep(&worker, 61);
        worker.submit(seat, input.clone(), now(&worker)).unwrap();
        let concede = request(&worker, seat, 62, CommandBody::Concede);
        worker.submit(seat, concede.clone(), now(&worker)).unwrap();
        assert_eq!(
            command_status(&worker, seat, &input),
            CommandExecutionStatus::Pending
        );
        assert_eq!(
            command_status(&worker, seat, &concede),
            CommandExecutionStatus::Pending
        );
        worker.advance(now(&worker)).unwrap();
        assert_eq!(
            command_status(&worker, seat, &input),
            CommandExecutionStatus::DiscardedByDeparture
        );
        assert_eq!(
            command_status(&worker, seat, &concede),
            CommandExecutionStatus::ConsumedByRules
        );
        assert!(worker.host.outstanding.is_empty());
        assert_eq!(worker.game.journal().len(), 1);
        drop(worker);
        let recovered = Worker::recover(&temp.pod(), catalog(), "a1", Instant::now()).unwrap();
        assert_eq!(
            command_status(&recovered, seat, &input),
            CommandExecutionStatus::DiscardedByDeparture
        );
        assert_eq!(
            command_status(&recovered, seat, &concede),
            CommandExecutionStatus::ConsumedByRules
        );
    }
    #[test]
    fn rejected_and_cancelled_execution_have_no_core_identity() {
        let (_temp, mut worker) = create();
        settle(&mut worker);
        let (seat, input) = keep(&worker, 63);
        worker
            .cancel_queued(seat, input.clone(), now(&worker))
            .unwrap();
        assert_eq!(
            command_status(&worker, seat, &input),
            CommandExecutionStatus::Cancelled
        );
        assert!(worker.host.outstanding.is_empty());
        let (_, mut stale) = keep(&worker, 64);
        stale.expected_revision = Counter::new(0);
        worker.submit(seat, stale.clone(), now(&worker)).unwrap();
        assert_eq!(
            command_status(&worker, seat, &stale),
            CommandExecutionStatus::Rejected {
                reason: RejectionReason::StaleRevision
            }
        );
        assert!(worker.host.outstanding.is_empty());
        assert!(worker.game.journal().is_empty());
    }
    #[test]
    fn failed_consumption_transaction_keeps_pending_execution_and_core_state() {
        let (temp, mut worker) = create();
        settle(&mut worker);
        let (seat, input) = keep(&worker, 65);
        worker.submit(seat, input.clone(), now(&worker)).unwrap();
        let before = worker.head.clone();
        let db = rusqlite::Connection::open(temp.pod().join("match.sqlite3")).unwrap();
        db.execute_batch("CREATE TRIGGER fail_execution BEFORE UPDATE ON execution BEGIN SELECT RAISE(ABORT,'injected'); END;").unwrap();
        assert!(worker.advance(now(&worker)).is_err());
        assert!(worker.halted);
        assert_eq!(worker.head, before);
        assert_eq!(worker.state_sha256().unwrap(), before.state_sha256);
        assert_eq!(
            command_status(&worker, seat, &input),
            CommandExecutionStatus::Pending
        );
        assert_eq!(worker.host.outstanding.len(), 1);
        db.execute_batch("DROP TRIGGER fail_execution;").unwrap();
        drop(db);
        drop(worker);
        let mut recovered = Worker::recover(&temp.pod(), catalog(), "a1", Instant::now()).unwrap();
        assert_eq!(
            command_status(&recovered, seat, &input),
            CommandExecutionStatus::Pending
        );
        recovered.advance(now(&recovered)).unwrap();
        assert_eq!(
            command_status(&recovered, seat, &input),
            CommandExecutionStatus::ConsumedByRules
        );
    }
    #[test]
    fn recovery_rejects_forged_consumption_facts_and_execution_dispositions() {
        for mutation in 0..4 {
            let (temp, mut worker) = create();
            settle(&mut worker);
            let (seat, input) = keep(&worker, 66);
            worker.submit(seat, input.clone(), now(&worker)).unwrap();
            worker.advance(now(&worker)).unwrap();
            drop(worker);
            let db = rusqlite::Connection::open(temp.pod().join("match.sqlite3")).unwrap();
            if mutation == 3 {
                db.execute(
                    "UPDATE execution SET status=?1 WHERE seat=?2 AND command_id=?3",
                    rusqlite::params![
                        canonical(&CommandExecutionStatus::Pending).unwrap(),
                        seat,
                        input.command_id
                    ],
                )
                .unwrap();
            } else {
                let (seq, body): (i64, String) = db
                    .query_row(
                        "SELECT sequence,body FROM records ORDER BY sequence DESC LIMIT 1",
                        [],
                        |r| Ok((r.get(0)?, r.get(1)?)),
                    )
                    .unwrap();
                let mut v: serde_json::Value = serde_json::from_str(&body).unwrap();
                match mutation {
                    0 => v["record"]["action"]["operation"]["processed"] = serde_json::json!([]),
                    1 => {
                        v["record"]["action"]["operation"]["processed"][0]["cause"] =
                            "departure_step".into()
                    }
                    2 => {
                        v["record"]["execution_updates"][0]["status"] =
                            serde_json::json!({"status":"discarded_by_departure"})
                    }
                    _ => unreachable!(),
                }
                db.execute(
                    "UPDATE records SET body=?1 WHERE sequence=?2",
                    rusqlite::params![canonical(&v).unwrap(), seq],
                )
                .unwrap();
            }
            drop(db);
            assert!(
                Worker::recover(&temp.pod(), catalog(), "a1", Instant::now()).is_err(),
                "mutation {mutation}"
            );
        }
    }
    #[test]
    fn recovery_rejects_unknown_fields_on_tagged_unit_variants() {
        for mutation in 0..2 {
            let (temp, mut worker) = create();
            worker.advance(now(&worker)).unwrap();
            drop(worker);
            mutate_record(&temp.pod(), |value| {
                let target = if mutation == 0 {
                    &mut value["record"]["action"]["operation"]["expected_return"]
                } else {
                    &mut value["record"]["after_host"]["phase"]
                };
                assert!(target.is_object());
                target["ignored_override"] = true.into();
            });
            assert!(Worker::recover(&temp.pod(), catalog(), "a1", Instant::now()).is_err());
        }
    }
}
