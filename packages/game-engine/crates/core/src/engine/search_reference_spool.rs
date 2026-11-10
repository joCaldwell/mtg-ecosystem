//! TEST HOST ONLY: complete old-owned Search proof, with one generic public
//! View serialized per original history unit into a private append-only file.
//! Prefix comparisons remain complete and potentially quadratic proof I/O.
use super::*;
use std::{
    fs::{self, File, OpenOptions},
    io::{self, BufReader, BufWriter, Read, Seek, SeekFrom, Write},
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
const BLOCK: usize = 64 * 1024;
static NEXT: AtomicU64 = AtomicU64::new(0);
fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
fn text(error: impl std::fmt::Display) -> String {
    error.to_string()
}
struct Directory(PathBuf);
impl Directory {
    fn new() -> io::Result<Self> {
        Self::under(&std::env::temp_dir())
    }
    fn under(base: &Path) -> io::Result<Self> {
        plain_ancestry(base)?;
        for _ in 0..100 {
            let path = base.join(format!(
                "mtg-query-proof-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::DirBuilder::new().mode(0o700).create(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
                Err(e) => return Err(e),
            }
        }
        Err(invalid("private query proof directory collisions"))
    }
    fn cleanup(self) -> io::Result<()> {
        // Success is not published until owned private files are removed.
        // Drop remains the best-effort error/unwind fallback only.
        plain_ancestry(&self.0)?;
        fs::remove_dir_all(&self.0)
    }
    fn create(&self, name: &str) -> io::Result<File> {
        if !matches!(
            Path::new(name).components().collect::<Vec<_>>().as_slice(),
            [Component::Normal(_)]
        ) {
            return Err(invalid("private proof file name must be plain"));
        }
        OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(self.0.join(name))
    }
}
fn plain_ancestry(path: &Path) -> io::Result<()> {
    if !path.is_absolute() {
        return Err(invalid("private proof parent must be absolute"));
    }
    let mut current = PathBuf::new();
    for component in path.components() {
        match component {
            Component::RootDir | Component::Normal(_) => current.push(component.as_os_str()),
            _ => return Err(invalid("private proof parent contains nonplain components")),
        }
        let metadata = fs::symlink_metadata(&current)?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(invalid("private proof ancestry is not plain directories"));
        }
    }
    Ok(())
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

// Opt-in TEST HOST observer only. Inclusive parent/child clocks overlap; totals
// are logical proof I/O, never CPU attribution or an uninstrumented cost claim.
const PROFILE_RECORDS: u64 = 16_384;
const PROFILE_LINE: usize = 1_024;
const PROFILE_BYTES: u64 = 16 * 1024 * 1024;
const PROFILE_SCHEMA: &str = "legal-proof-phase-events-v1";
#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub(in crate::engine) enum ProofKind {
    Root,
    Firstwrite,
}
#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub(in crate::engine) enum QueryKind {
    Observe,
    Decision,
    Search,
}
#[derive(Clone, Copy, Serialize)]
struct ProofScope {
    case: &'static str,
    viewer: Option<u8>,
    query_kind: Option<QueryKind>,
    quantum: Option<u64>,
    prefix_ordinal: Option<u64>,
    units_done: Option<u64>,
    history_entries: Option<u64>,
    proof_kind: Option<ProofKind>,
}
#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub(in crate::engine) enum ProofPhase {
    RootPrepare,
    InitialGenericStateOracle,
    QueryProof,
    FirstWriteActionSelection,
    BudgetFirstWrite,
    FirstWriteGenericStateOracle,
    SnapshotProof,
    ExportAndReceipt,
    StandaloneRestoreInclusive,
    ReadReceiptAndStateDigestValidation,
    IndependentAcceptedInputReplay,
    ReplayGenericStateOracle,
    FinalRootDigest,
    QuantumSetup,
    LazyZero,
    ReferenceResume,
    ActualResume,
    MetadataConversion,
    ReferenceSerializeWriteHash,
    ActualSerializeCompareHash,
    FinishEofCountDigest,
    SparseColdProof,
    SparseColdSave,
    SparseSavedEnvelopeComparison,
    SparseColdRestore,
    RestoredScratch,
    CompletedResult,
    CheckedDirectoryCleanup,
}
impl ProofPhase {
    fn group(self) -> Option<usize> {
        match self {
            Self::ReferenceResume => Some(0),
            Self::ActualResume => Some(1),
            Self::MetadataConversion => Some(2),
            Self::ReferenceSerializeWriteHash => Some(3),
            Self::ActualSerializeCompareHash => Some(4),
            Self::FinishEofCountDigest => Some(5),
            Self::SparseColdProof => Some(6),
            Self::CheckedDirectoryCleanup => Some(7),
            _ => None,
        }
    }
}
trait ProofSink: Write {
    fn sync(&mut self) -> io::Result<()>;
    fn rollback_terminal(&mut self, committed_bytes: u64) -> io::Result<()>;
}
impl ProofSink for File {
    fn sync(&mut self) -> io::Result<()> {
        self.sync_all()
    }
    fn rollback_terminal(&mut self, committed_bytes: u64) -> io::Result<()> {
        self.set_len(committed_bytes)?;
        if self.seek(SeekFrom::Start(committed_bytes))? != committed_bytes {
            return Err(invalid("profile terminal rollback position mismatch"));
        }
        self.sync_all()
    }
}
pub(in crate::engine) struct LegalProofProfile {
    sink: Box<dyn ProofSink>,
    clock: Box<dyn FnMut() -> Result<u64, String>>,
    scope: ProofScope,
    sampled: bool,
    totals: [[u64; 3]; 8],
    seq: u64,
    bytes: u64,
    last_at: u64,
    failed: bool,
    finished: bool,
    operation_failed: bool,
    active: u64,
    last_checkpoint: Option<u64>,
}
struct PhaseStart {
    at: u64,
    scope: ProofScope,
    sampled: bool,
}
impl LegalProofProfile {
    // Raw Unix segments are checked BEFORE Path::components normalizes `.`.
    pub(in crate::engine) fn open_under(base: &Path, case: &str) -> Result<Self, String> {
        use std::os::unix::{ffi::OsStrExt, fs::PermissionsExt};
        if base
            .as_os_str()
            .as_bytes()
            .split(|b| *b == b'/')
            .any(|part| part == b"." || part == b"..")
        {
            return Err("profile base has dot components".into());
        }
        plain_ancestry(base).map_err(text)?;
        if fs::symlink_metadata(base)
            .map_err(text)?
            .permissions()
            .mode()
            & 0o7777
            != 0o700
        {
            return Err("profile base must be mode0700".into());
        }
        let case = match case {
            "ordinary_4" => "ordinary_4",
            "ordinary_3" => "ordinary_3",
            "ordinary_2" => "ordinary_2",
            "dense_4" => "dense_4",
            "dense_3" => "dense_3",
            "dense_2" => "dense_2",
            _ => return Err("undeclared profile case".into()),
        };
        let child = base.join("legal-proof-phase-v1");
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&child)
            .map_err(text)?;
        let metadata = fs::symlink_metadata(&child).map_err(text)?;
        if !metadata.is_dir()
            || metadata.file_type().is_symlink()
            || metadata.permissions().mode() & 0o7777 != 0o700
        {
            return Err("profile child must be plain mode0700 directory".into());
        }
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(child.join("events.ndjson"))
            .map_err(text)?;
        let metadata = file.metadata().map_err(text)?;
        if !metadata.is_file() || metadata.permissions().mode() & 0o7777 != 0o600 {
            return Err("profile events must be regular mode0600 file".into());
        }
        // Retained sidecar deliberately has NO scratch owner/Drop cleanup.
        let origin = std::time::Instant::now();
        Self::new(
            Box::new(file),
            Box::new(move || u64::try_from(origin.elapsed().as_nanos()).map_err(text)),
            case,
        )
    }
    fn new(
        sink: Box<dyn ProofSink>,
        clock: Box<dyn FnMut() -> Result<u64, String>>,
        case: &'static str,
    ) -> Result<Self, String> {
        let mut profile = Self {
            sink,
            clock,
            scope: ProofScope {
                case,
                viewer: None,
                query_kind: None,
                quantum: None,
                prefix_ordinal: None,
                units_done: None,
                history_entries: None,
                proof_kind: Some(ProofKind::Root),
            },
            sampled: true,
            totals: [[0; 3]; 8],
            seq: 0,
            bytes: 0,
            last_at: 0,
            failed: false,
            finished: false,
            operation_failed: false,
            active: 0,
            last_checkpoint: None,
        };
        profile.emit(
            serde_json::json!({"schema":PROFILE_SCHEMA,"kind":"header","seq":0,
            "build_id":BUILD_ID,"collection_scope":"root_proofs_only","instrumentation":true}),
        )?;
        Ok(profile)
    }
    pub(in crate::engine) fn requested(
        value: Option<&std::ffi::OsStr>,
        guard: Option<&std::ffi::OsStr>,
    ) -> Result<bool, String> {
        match value {
            None => Ok(false),
            Some(value) if value == "1" && guard == Some(std::ffi::OsStr::new("120s-2gib-v1")) => {
                Ok(true)
            }
            _ => Err("proof profile requires exactly1 and existing120s-2gib-v1 guard".into()),
        }
    }
    fn emit(&mut self, value: serde_json::Value) -> Result<(), String> {
        if self.failed || self.finished {
            return Err("profile is closed or poisoned".into());
        }
        let result = (|| {
            let mut line = serde_json::to_vec(&value).map_err(text)?;
            line.push(b'\n');
            let next_seq = self.seq.checked_add(1).ok_or("profile sequence overflow")?;
            let next_bytes = self
                .bytes
                .checked_add(u64::try_from(line.len()).map_err(text)?)
                .ok_or("profile size overflow")?;
            if next_seq > PROFILE_RECORDS || line.len() > PROFILE_LINE || next_bytes > PROFILE_BYTES
            {
                return Err("profile bounds exceeded".into());
            }
            self.sink.write_all(&line).map_err(text)?;
            self.seq = next_seq;
            self.bytes = next_bytes;
            Ok(())
        })();
        if result.is_err() {
            self.failed = true;
        }
        result
    }
    fn now(&mut self) -> Result<u64, String> {
        if self.failed || self.finished {
            return Err("profile is closed or poisoned".into());
        }
        let at = (self.clock)().inspect_err(|_| self.failed = true)?;
        if at < self.last_at {
            self.failed = true;
            return Err("profile clock regressed".into());
        }
        self.last_at = at;
        Ok(at)
    }
    fn sync(&mut self) -> Result<(), String> {
        let result = self
            .sink
            .flush()
            .and_then(|()| self.sink.sync())
            .map_err(text);
        if result.is_err() {
            self.failed = true;
        }
        result
    }
    fn begin(&mut self, phase: ProofPhase) -> Result<PhaseStart, String> {
        if self.sampled {
            let at = self.now()?;
            self.emit(serde_json::json!({"schema":PROFILE_SCHEMA,"kind":"begin","seq":self.seq,
                "scope":self.scope,"phase":phase,"at_ns":at,"duration_ns":null,"outcome":null,"bytes":null}))?;
        }
        self.active = self.active.checked_add(1).ok_or_else(|| {
            self.failed = true;
            "profile nesting overflow".to_string()
        })?;
        // Publication precedes the operation clock (including in sampled mode).
        Ok(PhaseStart {
            at: self.now()?,
            scope: self.scope,
            sampled: self.sampled,
        })
    }
    fn end(
        &mut self,
        start: PhaseStart,
        phase: ProofPhase,
        ok: bool,
        bytes: Option<u64>,
    ) -> Result<(), String> {
        let at = self.now()?; // first action after operation, before formatting/counters
        let result = (|| {
            let duration = at.checked_sub(start.at).ok_or("profile elapsed overflow")?;
            self.active = self.active.checked_sub(1).ok_or("profile unmatched end")?;
            if ok && let Some(group) = phase.group() {
                let total = &mut self.totals[group];
                total[0] = total[0].checked_add(1).ok_or("profile calls overflow")?;
                total[1] = total[1]
                    .checked_add(duration)
                    .ok_or("profile duration overflow")?;
                total[2] = total[2]
                    .checked_add(bytes.unwrap_or(0))
                    .ok_or("profile bytes overflow")?;
            }
            if start.sampled {
                self.emit(
                    serde_json::json!({"schema":PROFILE_SCHEMA,"kind":"end","seq":self.seq,
                    "scope":start.scope,"phase":phase,"at_ns":at,"duration_ns":duration,
                    "outcome":if ok {"ok"} else {"error"},"bytes":if ok {bytes} else {None}}),
                )?;
            }
            if matches!(phase, ProofPhase::QueryProof) {
                self.sync()?;
            }
            Ok(())
        })();
        if !ok {
            self.operation_failed = true;
        }
        if result.is_err() {
            self.failed = true;
        }
        result
    }
    pub(in crate::engine) fn query(&mut self, viewer: PlayerId, kind: QueryKind) {
        self.scope.viewer = Some(viewer.0);
        self.scope.query_kind = Some(kind);
        self.scope.quantum = None;
        self.scope.prefix_ordinal = None;
        self.scope.units_done = None;
        self.scope.history_entries = None;
        self.sampled = true;
    }
    pub(in crate::engine) fn budget_quantum(&mut self, quantum: u64) {
        self.root(ProofKind::Firstwrite);
        self.scope.quantum = Some(quantum);
    }
    pub(in crate::engine) fn root(&mut self, kind: ProofKind) {
        self.scope = ProofScope {
            case: self.scope.case,
            viewer: None,
            query_kind: None,
            quantum: None,
            prefix_ordinal: None,
            units_done: None,
            history_entries: None,
            proof_kind: Some(kind),
        };
        self.sampled = true;
    }
    fn quantum(&mut self, quantum: u64, entries: u64) {
        self.scope.quantum = Some(quantum);
        self.scope.history_entries = Some(entries);
        self.scope.prefix_ordinal = Some(0);
        self.scope.units_done = Some(0);
        self.totals = [[0; 3]; 8];
        self.last_checkpoint = None;
        self.sampled = true;
    }
    fn prefix(&mut self, ordinal: u64, units: u64, cold: bool, final_result: bool) {
        self.scope.prefix_ordinal = Some(ordinal);
        self.scope.units_done = Some(units);
        self.sampled = ordinal == 0 || ordinal.is_power_of_two() || cold || final_result;
    }
    fn checkpoint(&mut self) -> Result<(), String> {
        let ordinal = self
            .scope
            .prefix_ordinal
            .ok_or("profile checkpoint lacks prefix")?;
        if !self.sampled || self.last_checkpoint == Some(ordinal) {
            return Ok(());
        }
        let at = self.now()?;
        let totals: Vec<_> = self
            .totals
            .iter()
            .enumerate()
            .filter(|(_, t)| t[0] != 0)
            .map(|(id, t)| [id as u64, t[0], t[1], t[2]])
            .collect();
        self.emit(
            serde_json::json!({"schema":PROFILE_SCHEMA,"kind":"checkpoint","seq":self.seq,
            "scope":self.scope,"at_ns":at,"last_completed_prefix":ordinal,"totals":totals}),
        )?;
        self.sync()?;
        self.last_checkpoint = Some(ordinal);
        Ok(())
    }
    pub(in crate::engine) fn finish(&mut self, ok: bool) -> Result<(), String> {
        if ok && (self.operation_failed || self.active != 0) {
            return Err("failed proof cannot complete collection".into());
        }
        let at = self.now()?;
        let committed_bytes = self.bytes;
        let committed_seq = self.seq;
        let result = self
            .emit(
                serde_json::json!({"schema":PROFILE_SCHEMA,"kind":"terminal","seq":self.seq,
            "at_ns":at,"collection_outcome":if ok {"root_collection_complete"} else {"error"}}),
            )
            .and_then(|()| self.sync());
        if let Err(primary) = result {
            self.failed = true;
            // This file is exclusively owned. Remove ONLY the attempted terminal,
            // including a complete accepted line followed by flush/sync failure.
            // Retain the preceding receipts and the primary error. Rollback and
            // its sync can themselves fail; native/outer success is still required.
            return match self.sink.rollback_terminal(committed_bytes) {
                Ok(()) => {
                    self.bytes = committed_bytes;
                    self.seq = committed_seq;
                    Err(primary)
                }
                Err(rollback) => Err(format!("{primary}; terminal rollback failed: {rollback}")),
            };
        }
        self.finished = true;
        Ok(())
    }
}
// None calls only the original operation: no clock, collector or sink access.
pub(in crate::engine) fn proof_phase<T>(
    profile: Option<&mut LegalProofProfile>,
    phase: ProofPhase,
    operation: impl FnOnce(Option<&mut LegalProofProfile>) -> Result<T, String>,
) -> Result<T, String> {
    proof_phase_bytes(profile, phase, |p| operation(p).map(|v| (v, None)))
}
fn proof_phase_bytes<T>(
    mut profile: Option<&mut LegalProofProfile>,
    phase: ProofPhase,
    operation: impl FnOnce(Option<&mut LegalProofProfile>) -> Result<(T, Option<u64>), String>,
) -> Result<T, String> {
    let start = profile.as_deref_mut().map(|p| p.begin(phase)).transpose()?;
    let result = operation(profile.as_deref_mut());
    if let (Some(p), Some(start)) = (profile, start) {
        // Read the end clock before inspecting successful byte receipts.
        p.end(
            start,
            phase,
            result.is_ok(),
            result.as_ref().ok().and_then(|v| v.1),
        )?;
    }
    result.map(|v| v.0)
}

// Generic public serializer ONLY. No StoredView canonical writer/row memo or
// PermittedHistory serializer is used as the reference history oracle.
struct HistorySpool {
    file: File,
    ends: Vec<u64>,
    poisoned: bool,
}
impl HistorySpool {
    fn new(directory: &Directory) -> io::Result<Self> {
        Ok(Self {
            file: directory.create("history.json-fragments")?,
            ends: vec![],
            poisoned: false,
        })
    }
    fn len(&self) -> usize {
        self.ends.len()
    }
    fn append(&mut self, view: &PermittedView) -> Result<(), String> {
        if self.poisoned {
            return Err("poisoned reference history".into());
        }
        let bytes = canonical(view)?;
        let result = (|| -> io::Result<u64> {
            let before = self.ends.last().copied().unwrap_or(0);
            if self.file.metadata()?.len() != before || self.file.seek(SeekFrom::End(0))? != before
            {
                return Err(invalid("reference history length changed"));
            }
            let mut writer = BufWriter::with_capacity(BLOCK, &mut self.file);
            if !self.ends.is_empty() {
                writer.write_all(b",")?;
            }
            writer.write_all(bytes.as_bytes())?;
            writer.flush()?;
            drop(writer);
            let comma = u64::from(!self.ends.is_empty());
            let end = before
                .checked_add(comma)
                .and_then(|n| n.checked_add(bytes.len().try_into().ok()?))
                .ok_or_else(|| invalid("reference history length overflow"))?;
            if self.file.stream_position()? != end || self.file.metadata()?.len() != end {
                return Err(invalid("reference history append not exact"));
            }
            Ok(end)
        })();
        match result {
            Ok(end) => {
                self.ends.push(end);
                Ok(())
            }
            Err(e) => {
                self.poisoned = true;
                Err(text(e))
            }
        }
    }
    fn write_array(&mut self, writer: &mut dyn Write) -> Result<(), String> {
        if self.poisoned {
            return Err("poisoned reference history".into());
        }
        let end = self.ends.last().copied().unwrap_or(0);
        if self.file.metadata().map_err(text)?.len() != end {
            return Err("reference history appended/truncated".into());
        }
        if self.file.seek(SeekFrom::Start(0)).map_err(text)? != 0 {
            return Err("reference history rewind mismatch".into());
        }
        writer.write_all(b"[").map_err(text)?;
        copy_exact(&mut self.file, writer, end).map_err(text)?;
        let mut last = [0];
        if read_retry(&mut self.file, &mut last).map_err(text)? != 0 {
            return Err("reference history extra bytes".into());
        }
        writer.write_all(b"]").map_err(text)
    }
}
fn read_retry(reader: &mut impl Read, buf: &mut [u8]) -> io::Result<usize> {
    loop {
        match reader.read(buf) {
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Ok(n) if n > buf.len() => return Err(invalid("reader overreported bytes")),
            other => return other,
        }
    }
}
fn copy_exact(reader: &mut impl Read, writer: &mut dyn Write, mut bytes: u64) -> io::Result<()> {
    let mut block = [0; BLOCK];
    while bytes != 0 {
        let size =
            usize::try_from(bytes.min(BLOCK as u64)).map_err(|_| invalid("copy size overflow"))?;
        let n = read_retry(reader, &mut block[..size])?;
        if n == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "truncated reference prefix",
            ));
        }
        writer.write_all(&block[..n])?;
        bytes -= n as u64;
    }
    Ok(())
}

// The skeleton contains all ordinary fields and an EMPTY history placeholder.
// Replacement is exact-path and must happen exactly once, never silently skip.
#[derive(Default)]
struct Replacement {
    hits: u8,
}
impl Replacement {
    fn hit(&mut self) -> Result<(), String> {
        self.hits = self
            .hits
            .checked_add(1)
            .ok_or("replacement count overflow")?;
        if self.hits != 1 {
            return Err("duplicate history replacement".into());
        }
        Ok(())
    }
    fn finish(&self) -> Result<(), String> {
        if self.hits == 1 {
            Ok(())
        } else {
            Err("missing history replacement".into())
        }
    }
}
fn replaced(
    value: &serde_json::Value,
    path: &[&str],
    out: &mut dyn Write,
    history: &mut dyn FnMut(&mut dyn Write) -> Result<(), String>,
) -> Result<(), String> {
    fn walk(
        value: &serde_json::Value,
        path: &[&str],
        out: &mut dyn Write,
        history: &mut dyn FnMut(&mut dyn Write) -> Result<(), String>,
        guard: &mut Replacement,
    ) -> Result<(), String> {
        if path.is_empty() {
            if !value.as_array().is_some_and(Vec::is_empty) {
                return Err("history placeholder must be empty array".into());
            }
            guard.hit()?;
            return history(out);
        }
        let map = value.as_object().ok_or("history path is not an object")?;
        if !map.contains_key(path[0]) {
            return Err("missing history path".into());
        }
        let mut keys = map.keys().collect::<Vec<_>>();
        keys.sort_unstable();
        out.write_all(b"{").map_err(text)?;
        for (i, key) in keys.into_iter().enumerate() {
            if i != 0 {
                out.write_all(b",").map_err(text)?;
            }
            serde_json::to_writer(&mut *out, key).map_err(text)?;
            out.write_all(b":").map_err(text)?;
            if key == path[0] {
                walk(&map[key], &path[1..], out, history, guard)?;
            } else {
                serde_json::to_writer(&mut *out, &map[key]).map_err(text)?;
            }
        }
        out.write_all(b"}").map_err(text)
    }
    let mut guard = Replacement::default();
    walk(value, path, out, history, &mut guard)?;
    guard.finish()
}
fn actual_data(data: &WorkData) -> Result<serde_json::Value, String> {
    if matches!(data.task, Task::Initialize) {
        return serde_json::to_value(data).map_err(text);
    }
    let WorkData {
        query,
        task,
        units_done,
    } = data;
    let Task::Search {
        builder,
        observation,
        history: _,
        decks,
    } = task
    else {
        return Err("non-Search scratch".into());
    };
    serde_json::to_value(WorkData {
        query: query.clone(),
        task: Task::Search {
            builder: builder.clone(),
            observation: observation.clone(),
            history: PermittedHistory::default(),
            decks: decks.clone(),
        },
        units_done: *units_done,
    })
    .map_err(text)
}
fn actual_result(context: &SearchContext) -> Result<serde_json::Value, String> {
    let SearchContext {
        context_version,
        observation,
        permitted_history: _,
        open_decklists,
        catalog_digest,
        profile_id,
        sampling_seed,
    } = context;
    let skeleton = SearchContext {
        context_version: *context_version,
        observation: observation.clone(),
        permitted_history: PermittedHistory::default(),
        open_decklists: open_decklists.clone(),
        catalog_digest: catalog_digest.clone(),
        profile_id: profile_id.clone(),
        sampling_seed: sampling_seed.clone(),
    };
    Ok(serde_json::json!({"context":skeleton}))
}
fn write_actual_history(history: &PermittedHistory, mut out: &mut dyn Write) -> Result<(), String> {
    out.write_all(b"[").map_err(text)?;
    for index in 0..history.len() {
        if index != 0 {
            out.write_all(b",").map_err(text)?;
        }
        // This is the ACTUAL side only. The old reference never calls this.
        history
            .stored(index)
            .ok_or("missing actual history entry")?
            .write_canonical(&mut out)?;
    }
    out.write_all(b"]").map_err(text)
}

struct Counted<W> {
    inner: W,
    hash: Sha256Sink,
    bytes: u64,
}
impl<W: Write> Write for Counted<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let n = self.inner.write(buf)?;
        if n > buf.len() {
            return Err(invalid("writer overreported bytes"));
        }
        self.bytes = self
            .bytes
            .checked_add(n as u64)
            .ok_or_else(|| invalid("proof byte count overflow"))?;
        self.hash.update(&buf[..n]);
        Ok(n)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}
struct Equal<R> {
    reader: R,
    hash: Sha256Sink,
    bytes: u64,
    poisoned: bool,
    // Serializer fragments share this scratch; only the returned read prefix
    // participates in comparison. Every byte/count/hash/EOF check stays intact.
    block: [u8; BLOCK],
}
impl<R: Read> Equal<R> {
    fn new(reader: R) -> Self {
        Self {
            reader,
            hash: Sha256Sink::new(),
            bytes: 0,
            poisoned: false,
            block: [0; BLOCK],
        }
    }
    fn finish(mut self) -> Result<(u64, String), String> {
        if self.poisoned {
            return Err("poisoned byte comparison".into());
        }
        let mut next = [0];
        if read_retry(&mut self.reader, &mut next).map_err(text)? != 0 {
            return Err("actual ended before complete reference EOF".into());
        }
        Ok((self.bytes, self.hash.finalize_hex()))
    }
}
impl<R: Read> Write for Equal<R> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.poisoned {
            return Err(invalid("poisoned byte comparison"));
        }
        let result = (|| {
            let mut done = 0;
            while done < buf.len() {
                let size = (buf.len() - done).min(BLOCK);
                let n = read_retry(&mut self.reader, &mut self.block[..size])?;
                if n == 0 {
                    return Err(invalid("actual contains bytes beyond reference EOF"));
                }
                if self.block[..n] != buf[done..done + n] {
                    return Err(invalid("complete query bytes differ"));
                }
                self.bytes = self
                    .bytes
                    .checked_add(n as u64)
                    .ok_or_else(|| invalid("comparison byte count overflow"))?;
                self.hash.update(&buf[done..done + n]);
                done += n;
            }
            Ok(buf.len())
        })();
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }
    fn flush(&mut self) -> io::Result<()> {
        if self.poisoned {
            Err(invalid("poisoned byte comparison"))
        } else {
            Ok(())
        }
    }
}
fn compare_bytes(
    directory: &Directory,
    reference: impl FnOnce(&mut dyn Write) -> Result<(), String>,
    actual: impl FnOnce(&mut dyn Write) -> Result<(), String>,
) -> Result<(), String> {
    compare_bytes_with_profile(directory, reference, actual, None)
}
fn compare_bytes_with_profile(
    directory: &Directory,
    reference: impl FnOnce(&mut dyn Write) -> Result<(), String>,
    actual: impl FnOnce(&mut dyn Write) -> Result<(), String>,
    mut profile: Option<&mut LegalProofProfile>,
) -> Result<(), String> {
    let (expected, file) = proof_phase_bytes(
        profile.as_deref_mut(),
        ProofPhase::ReferenceSerializeWriteHash,
        |_| {
            let path = directory.0.join("reference.json");
            let file = match directory.create("reference.json") {
                Ok(file) => file,
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                    let metadata = fs::symlink_metadata(&path).map_err(text)?;
                    if metadata.file_type().is_symlink() || !metadata.is_file() {
                        return Err("reference proof file is not plain".into());
                    }
                    OpenOptions::new()
                        .write(true)
                        .truncate(true)
                        .open(&path)
                        .map_err(text)?
                }
                Err(error) => return Err(text(error)),
            };
            let mut writer = Counted {
                inner: BufWriter::with_capacity(BLOCK, file),
                hash: Sha256Sink::new(),
                bytes: 0,
            };
            reference(&mut writer)?;
            writer.flush().map_err(text)?;
            let expected = (writer.bytes, writer.hash.finalize_hex());
            drop(writer.inner);
            let file = File::open(path).map_err(text)?;
            if file.metadata().map_err(text)?.len() != expected.0 {
                return Err("flushed reference length mismatch".into());
            }
            let bytes = expected.0;
            Ok(((expected, file), Some(bytes)))
        },
    )?;
    let mut comparison = Equal::new(BufReader::with_capacity(BLOCK, file));
    proof_phase_bytes(
        profile.as_deref_mut(),
        ProofPhase::ActualSerializeCompareHash,
        |_| {
            // Coalesce ACTUAL serializer fragments only; every full prefix still
            // reaches Equal. Check the final buffered tail before count/SHA/EOF.
            let mut actual_writer = BufWriter::with_capacity(BLOCK, &mut comparison);
            actual(&mut actual_writer)?;
            actual_writer.flush().map_err(text)?;
            drop(actual_writer);
            Ok(((), Some(comparison.bytes)))
        },
    )?;
    proof_phase_bytes(profile, ProofPhase::FinishEofCountDigest, |_| {
        comparison.flush().map_err(text)?;
        if comparison.finish()? != expected {
            return Err("complete comparison count/SHA mismatch".into());
        }
        Ok(((), Some(expected.0)))
    })
}

#[derive(Debug)]
enum ReferenceError {
    Query(QueryRejection),
    Proof(String),
}
struct Reference {
    initialized: bool,
    query: Query,
    builder: ObservationWork,
    observation: Option<Box<Observation>>,
    history: HistorySpool,
    decks: Vec<OpenDecklist>,
    units_done: u64,
    completed: bool,
}
#[derive(Serialize)]
#[serde(tag = "task", rename_all = "snake_case")]
enum OldTask<'a> {
    Initialize,
    Search {
        builder: &'a ObservationWork,
        observation: &'a Option<Box<Observation>>,
        history: &'a [PermittedView],
        decks: &'a [OpenDecklist],
    },
}
#[derive(Serialize)]
struct OldData<'a> {
    query: &'a Query,
    task: OldTask<'a>,
    units_done: u64,
}
#[derive(Serialize)]
struct OldContext<'a> {
    context_version: u32,
    observation: &'a Observation,
    permitted_history: &'a [PermittedView],
    open_decklists: &'a [OpenDecklist],
    catalog_digest: &'a str,
    profile_id: &'a str,
    sampling_seed: &'a str,
}
impl Reference {
    fn new(game: &Game, work: &Work, directory: &Directory) -> Result<Self, String> {
        let Query::Search { viewer, .. } = &work.data.query else {
            return Err("reference requires Search".into());
        };
        if work.data.units_done != 0 || !matches!(work.data.task, Task::Initialize) {
            return Err("reference must start at lazy zero".into());
        }
        // Keep the original eager header algorithm as the body oracle. Its
        // exposure now follows one explicit initialization unit in sidecar v16.
        Ok(Self {
            initialized: false,
            query: work.data.query.clone(),
            builder: ObservationWork {
                cursor: 0,
                view: game.blank_view(*viewer)?,
                decision: game.request_header(*viewer)?,
            },
            observation: None,
            history: HistorySpool::new(directory).map_err(text)?,
            decks: vec![],
            units_done: 0,
            completed: false,
        })
    }
    fn unit(&mut self, game: &Game) -> Result<bool, ReferenceError> {
        if !self.initialized {
            self.initialized = true;
            return Ok(false);
        }
        let Query::Search { viewer, .. } = &self.query else {
            return Err(ReferenceError::Query(QueryRejection::Invariant));
        };
        if self.observation.is_none() {
            self.observation = game
                .reference_observation_unit(&mut self.builder)
                .map_err(ReferenceError::Query)?
                .map(Box::new);
            return Ok(false);
        }
        let permitted = &game.state.knowledge.visibility[viewer.0 as usize].history;
        if self.history.len() < permitted.len() {
            // Exactly one complete original public entry; discard it immediately
            // after the generic serializer. Never build/retain an old-owned Vec.
            let public = permitted[self.history.len()].to_public();
            self.history
                .append(&public)
                .map_err(ReferenceError::Proof)?;
            return Ok(false);
        }
        if self.decks.len() < 4 {
            let player = PlayerId(self.decks.len() as u8);
            let mut definitions = BTreeMap::new();
            for card in game
                .prepared
                .config
                .cards
                .iter()
                .filter(|c| c.owner == player)
            {
                let count = definitions.entry(card.oracle_id.clone()).or_insert(0_u32);
                *count = count
                    .checked_add(1)
                    .ok_or(ReferenceError::Query(QueryRejection::Invariant))?;
            }
            self.decks.push(OpenDecklist {
                player,
                definitions,
            });
            return Ok(false);
        }
        Ok(true)
    }
    fn resume(&mut self, game: &Game, mut budget: Budget) -> Result<bool, ReferenceError> {
        while budget.consume() {
            self.units_done = self
                .units_done
                .checked_add(1)
                .ok_or(ReferenceError::Query(QueryRejection::Invariant))?;
            if self.unit(game)? {
                self.completed = true;
                return Ok(true);
            }
        }
        Ok(false)
    }
    fn data(&self) -> Result<serde_json::Value, String> {
        serde_json::to_value(OldData {
            query: &self.query,
            task: if self.initialized {
                OldTask::Search {
                    builder: &self.builder,
                    observation: &self.observation,
                    history: &[],
                    decks: &self.decks,
                }
            } else {
                OldTask::Initialize
            },
            units_done: self.units_done,
        })
        .map_err(text)
    }
    fn result(&self, game: &Game) -> Result<serde_json::Value, String> {
        if !self.completed {
            return Err("reference result before completion".into());
        }
        let Query::Search { sampling_seed, .. } = &self.query else {
            return Err("reference result query".into());
        };
        let observation = self
            .observation
            .as_deref()
            .ok_or("reference result observation")?;
        Ok(
            serde_json::json!({"context":OldContext {context_version:16,observation,permitted_history:&[],open_decklists:&self.decks,catalog_digest:&game.prepared.catalog_digest,profile_id:&game.prepared.config.profile,sampling_seed}}),
        )
    }
}
fn scratch(
    directory: &Directory,
    reference: &mut Reference,
    actual: &WorkData,
) -> Result<(), String> {
    scratch_with_profile(directory, reference, actual, None)
}
fn scratch_with_profile(
    directory: &Directory,
    reference: &mut Reference,
    actual: &WorkData,
    mut profile: Option<&mut LegalProofProfile>,
) -> Result<(), String> {
    let (old, new) = proof_phase(
        profile.as_deref_mut(),
        ProofPhase::MetadataConversion,
        |_| Ok((reference.data()?, actual_data(actual)?)),
    )?;
    if matches!(actual.task, Task::Initialize) {
        return compare_bytes_with_profile(
            directory,
            |out| serde_json::to_writer(out, &old).map_err(text),
            |out| serde_json::to_writer(out, &new).map_err(text),
            profile,
        );
    }
    let Task::Search { history, .. } = &actual.task else {
        return Err("actual scratch not Search".into());
    };
    compare_bytes_with_profile(
        directory,
        |out| {
            replaced(&old, &["task", "history"], out, &mut |out| {
                reference.history.write_array(out)
            })
        },
        |out| {
            replaced(&new, &["task", "history"], out, &mut |out| {
                write_actual_history(history, out)
            })
        },
        profile,
    )
}

pub(super) fn compare(game: &Game, query: Query) {
    compare_with_profile(game, query, None);
}
pub(super) fn compare_with_profile(
    game: &Game,
    query: Query,
    mut profile: Option<&mut LegalProofProfile>,
) {
    let before = game.state.clone();
    let trace = game.trace.clone();
    let state_sha256 = game.state_digest().unwrap();
    for quantum in [1, 64, u64::MAX] {
        if let Some(p) = profile.as_deref_mut() {
            let Query::Search { viewer, .. } = query else {
                unreachable!()
            };
            p.query(viewer, QueryKind::Search);
            p.quantum(
                quantum,
                u64::try_from(
                    game.state.knowledge.visibility[viewer.0 as usize]
                        .history
                        .len(),
                )
                .unwrap(),
            );
        }
        let (directory, mut work, mut reference) =
            proof_phase(profile.as_deref_mut(), ProofPhase::QuantumSetup, |_| {
                let directory = Directory::new().unwrap();
                let work = game.start_work(query.clone()).unwrap();
                let reference = Reference::new(game, &work, &directory).unwrap();
                Ok((directory, work, reference))
            })
            .unwrap();
        work = proof_phase(profile.as_deref_mut(), ProofPhase::LazyZero, |profile| {
            assert!(work.cache.effects.is_none());
            assert!(work.cache.last_object.is_none());
            let QueryResult::Incomplete {
                work: zero,
                required_minimum,
            } = game.resume_work(work, Budget::new(0))
            else {
                panic!("zero Search query budget")
            };
            assert_eq!(required_minimum, 1);
            work = zero;
            assert!(work.cache.effects.is_none());
            assert!(work.cache.last_object.is_none());
            scratch_with_profile(&directory, &mut reference, &work.data, profile)?;
            Ok(work)
        })
        .unwrap();
        if let Some(p) = profile.as_deref_mut() {
            p.checkpoint().unwrap();
        }
        let mut checkpoints = 0;
        let mut ordinal = 0_u64;
        loop {
            if let Some(p) = profile.as_deref_mut() {
                // Resume intervals carry the existing START cursor, never a
                // predicted successful unit count. Scratch/checkpoint scopes below
                // carry the returned cursor. Sampling does not change cold work.
                ordinal = ordinal.checked_add(1).unwrap();
                let cold = (quantum == 1 && [1, 5, 31, 129].contains(&ordinal))
                    || (quantum == 64 && checkpoints == 0);
                p.prefix(ordinal, reference.units_done, cold, false);
            }
            let start = profile
                .as_deref_mut()
                .map(|p| p.begin(ProofPhase::ReferenceResume).unwrap());
            let expected = reference.resume(game, Budget::new(quantum));
            if let (Some(p), Some(start)) = (profile.as_deref_mut(), start) {
                p.end(start, ProofPhase::ReferenceResume, expected.is_ok(), None)
                    .unwrap();
            }
            let start = profile
                .as_deref_mut()
                .map(|p| p.begin(ProofPhase::ActualResume).unwrap());
            let actual = game.resume_work(work, Budget::new(quantum));
            if let (Some(p), Some(start)) = (profile.as_deref_mut(), start) {
                p.end(
                    start,
                    ProofPhase::ActualResume,
                    !matches!(actual, QueryResult::Rejected(_)),
                    None,
                )
                .unwrap();
            }
            if let Some(p) = profile.as_deref_mut() {
                p.scope.units_done = Some(reference.units_done);
            }
            match (actual, expected) {
                (QueryResult::Complete(QueryValue::SearchContext(context)), Ok(true)) => {
                    if let Some(p) = profile.as_deref_mut() {
                        p.prefix(ordinal, reference.units_done, false, true);
                    }
                    proof_phase(
                        profile.as_deref_mut(),
                        ProofPhase::CompletedResult,
                        |mut profile| {
                            let (old, new) = proof_phase(
                                profile.as_deref_mut(),
                                ProofPhase::MetadataConversion,
                                |_| Ok((reference.result(game)?, actual_result(&context)?)),
                            )?;
                            compare_bytes_with_profile(
                                &directory,
                                |out| {
                                    replaced(
                                        &old,
                                        &["context", "permitted_history"],
                                        out,
                                        &mut |out| reference.history.write_array(out),
                                    )
                                },
                                |out| {
                                    replaced(
                                        &new,
                                        &["context", "permitted_history"],
                                        out,
                                        &mut |out| {
                                            write_actual_history(&context.permitted_history, out)
                                        },
                                    )
                                },
                                profile,
                            )
                        },
                    )
                    .unwrap();
                    break;
                }
                (QueryResult::Rejected(actual), Err(ReferenceError::Query(expected))) => {
                    assert_eq!(actual, expected);
                    break;
                }
                (
                    QueryResult::Incomplete {
                        work: next,
                        required_minimum,
                    },
                    Ok(false),
                ) => {
                    assert_eq!(required_minimum, 1);
                    scratch_with_profile(
                        &directory,
                        &mut reference,
                        &next.data,
                        profile.as_deref_mut(),
                    )
                    .unwrap();
                    work = next;
                    // Same absolute sparse checkpoints under the new initialization schedule.
                    if (quantum == 1 && [1, 5, 31, 129].contains(&reference.units_done))
                        || (quantum == 64 && checkpoints == 0)
                    {
                        work = proof_phase(
                            profile.as_deref_mut(),
                            ProofPhase::SparseColdProof,
                            |mut profile| {
                                let saved = proof_phase(
                                    profile.as_deref_mut(),
                                    ProofPhase::SparseColdSave,
                                    |_| game.save_work(&work),
                                )?;
                                let envelope = serde_json::json!({"work_version":16,"build_id":BUILD_ID,"catalog_digest":game.prepared.catalog_digest,"state_sha256":state_sha256,"data":reference.data().unwrap()});
                                proof_phase(
                                    profile.as_deref_mut(),
                                    ProofPhase::SparseSavedEnvelopeComparison,
                                    |profile| {
                                        compare_bytes_with_profile(
                                            &directory,
                                            |out| {
                                                replaced(&envelope, &["data", "task", "history"], out, &mut |out| {
                                                    reference.history.write_array(out)
                                                })
                                            },
                                            |out| out.write_all(saved.as_bytes()).map_err(text),
                                            profile,
                                        )
                                    },
                                )?;
                                let cold = proof_phase(
                                    profile.as_deref_mut(),
                                    ProofPhase::SparseColdRestore,
                                    |_| game.restore_work(&saved),
                                )?;
                                proof_phase(profile, ProofPhase::RestoredScratch, |profile| {
                                    scratch_with_profile(&directory, &mut reference, &cold.data, profile)
                                })?;
                                Ok(cold)
                            },
                        ).unwrap();
                        checkpoints += 1;
                    }
                    if let Some(p) = profile.as_deref_mut() {
                        p.checkpoint().unwrap();
                    }
                }
                (_, Err(ReferenceError::Proof(error))) => {
                    panic!("complete Search reference proof I/O failed:{error}")
                }
                _ => panic!("optimized Search changed reference completion/rejection unit"),
            }
        }
        proof_phase(
            profile.as_deref_mut(),
            ProofPhase::CheckedDirectoryCleanup,
            |_| {
                drop(reference);
                directory.cleanup().map_err(text)
            },
        )
        .unwrap();
        if let Some(p) = profile.as_deref_mut() {
            p.checkpoint().unwrap();
            p.sync().unwrap();
        }
    }
    assert_eq!(game.state, before);
    assert_eq!(game.trace, trace);
    assert_eq!(game.state_digest().unwrap(), state_sha256);
}

#[cfg(test)]
#[path = "search_reference_spool_tests.rs"]
mod tests;

// Receipt follows the strict reader's actual two complete reads. Only the one
// explicit rewind after first EOF resets it; stream_position(Current0) never
// erases bytes. No additional full-file verification scans are required.
struct ReceiptRead<R> {
    source: R,
    bytes: u64,
    hash: Sha256Sink,
    eof: bool,
    failed: bool,
    first: Option<(u64, String)>,
}
impl<R: Read + Seek> ReceiptRead<R> {
    fn new(mut source: R) -> io::Result<Self> {
        if source.stream_position()? != 0 {
            return Err(invalid("receipt reader must start at zero"));
        }
        Ok(Self {
            source,
            bytes: 0,
            hash: Sha256Sink::new(),
            eof: false,
            failed: false,
            first: None,
        })
    }
    fn finish(self) -> Result<(u64, String), String> {
        self.finish_with_source().map(|v| v.0)
    }
    fn finish_with_source(self) -> Result<((u64, String), R), String> {
        if self.failed || !self.eof {
            return Err("snapshot receipt requires successful complete EOF".into());
        }
        let first = self
            .first
            .ok_or("snapshot receipt requires both complete passes")?;
        let last = (self.bytes, self.hash.finalize_hex());
        if first != last {
            return Err("snapshot receipt passes differ".into());
        }
        Ok((last, self.source))
    }
}
impl<R: Read + Seek> Read for ReceiptRead<R> {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if self.failed {
            return Err(invalid("poisoned snapshot receipt reader"));
        }
        if out.is_empty() {
            return Ok(0);
        }
        let result = (|| {
            let n = self.source.read(out)?;
            if n > out.len() {
                return Err(invalid("snapshot reader overreported bytes"));
            }
            self.bytes = self
                .bytes
                .checked_add(n as u64)
                .ok_or_else(|| invalid("snapshot receipt byte count overflow"))?;
            self.hash.update(&out[..n]);
            self.eof = n == 0;
            Ok(n)
        })();
        if let Err(e) = &result
            && e.kind() != io::ErrorKind::Interrupted
        {
            self.failed = true;
        }
        result
    }
}
impl<R: Read + Seek> Seek for ReceiptRead<R> {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        if self.failed {
            return Err(invalid("poisoned snapshot receipt seek"));
        }
        let result = (|| match position {
            SeekFrom::Current(0) => {
                let at = self.source.stream_position()?;
                if at != self.bytes {
                    return Err(invalid(
                        "snapshot current position differs from counted bytes",
                    ));
                }
                Ok(at)
            }
            SeekFrom::Start(0) if self.eof && self.first.is_none() => {
                if self.source.seek(SeekFrom::Start(0))? != 0 {
                    return Err(invalid("snapshot rewind mismatch"));
                }
                let first = std::mem::replace(&mut self.hash, Sha256Sink::new());
                self.first = Some((self.bytes, first.finalize_hex()));
                self.bytes = 0;
                self.eof = false;
                Ok(0)
            }
            _ => Err(invalid(
                "snapshot receipt permits only complete first-pass rewind",
            )),
        })();
        if result.is_err() {
            self.failed = true;
        }
        result
    }
}
pub(super) fn restore_snapshot_from_private_file(game: &Game) -> Result<Game, String> {
    restore_snapshot_from_private_file_with_profile(game, None)
}
pub(super) fn restore_snapshot_from_private_file_with_profile(
    game: &Game,
    profile: Option<&mut LegalProofProfile>,
) -> Result<Game, String> {
    restore_snapshot_in_directory(game, Directory::new().map_err(text)?, profile)
}
fn restore_snapshot_in_directory(
    game: &Game,
    directory: Directory,
    mut profile: Option<&mut LegalProofProfile>,
) -> Result<Game, String> {
    let mut file = directory.create("snapshot.json").map_err(text)?;
    let written = proof_phase_bytes(profile.as_deref_mut(), ProofPhase::ExportAndReceipt, |_| {
        let written = {
            let mut writer = BufWriter::with_capacity(BLOCK, &mut file);
            let written = game.write_snapshot(&mut writer)?;
            writer.flush().map_err(text)?;
            written
        };
        if file.metadata().map_err(text)?.len() != written.bytes {
            return Err("written snapshot file length mismatch".into());
        }
        let bytes = written.bytes;
        Ok((written, Some(bytes)))
    })?;
    if file.seek(SeekFrom::Start(0)).map_err(text)? != 0 {
        return Err("snapshot file initial rewind mismatch".into());
    }
    let mut counted = ReceiptRead::new(&mut file).map_err(text)?;
    // This ordinary API performs the unchanged duplicate scan, typed decode,
    // admission, full State/invariant/trace and accepted-input replay proof.
    let restored = proof_phase(
        profile.as_deref_mut(),
        ProofPhase::StandaloneRestoreInclusive,
        |_| Game::restore_from_reader(game.prepared.as_ref().clone(), &mut counted),
    )?;
    proof_phase(
        profile.as_deref_mut(),
        ProofPhase::ReadReceiptAndStateDigestValidation,
        |_| {
            let (read, source) = counted.finish_with_source()?;
            if read != (written.bytes, written.snapshot_sha256) {
                return Err("read snapshot bytes differ from writer receipt".into());
            }
            if source.metadata().map_err(text)?.len() != written.bytes {
                return Err("snapshot file changed length during restore".into());
            }
            if restored.state_digest()? != written.state_sha256 {
                return Err("restored full State differs from writer receipt".into());
            }
            Ok(())
        },
    )?;
    proof_phase(profile, ProofPhase::CheckedDirectoryCleanup, |_| {
        drop(file);
        directory.cleanup().map_err(text)
    })?;
    Ok(restored)
}
