//! Host-only generic State oracle: retain generic non-history fields and one
//! generic history view, never a whole snapshot String or complete history Value.
//! Source values come from the proved snapshot exporter; this independent generic
//! re-encoder does not claim direct access to private raw State.
use super::{Game, GenericStateProof, Value, fs};
use mtg_contracts::{Sha256Sink, strict_read_seekable};
use serde::{
    Deserialize,
    de::{self, DeserializeSeed, IgnoredAny, MapAccess, SeqAccess, Visitor},
};
use std::{
    collections::BTreeMap,
    fmt,
    io::{self, BufWriter, Read, Seek, SeekFrom, Write},
    os::unix::fs::OpenOptionsExt,
};

// This observer exists only in the cfg(test) diagnostic host. Inclusive parent
// and child intervals overlap; their totals must never be summed as CPU time.
use std::{
    os::unix::{
        ffi::OsStrExt,
        fs::{DirBuilderExt, PermissionsExt},
    },
    path::{Component, Path},
    time::Instant,
};

const PROFILE_SCHEMA: &str = "generic-proof-phase-events-v1";
const MAX_RECORDS: u64 = 16_384;
const MAX_LINE: usize = 1_024;
const MAX_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Clone, Copy, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Operand {
    A,
    B,
}
#[derive(Clone, Copy, serde::Serialize)]
pub(super) struct ProofScope {
    living: u8,
    equality_ordinal: u64,
    operand: Option<Operand>,
}
impl ProofScope {
    pub(super) fn root(living: usize) -> Result<Self, String> {
        if ![4, 3, 2].contains(&living) {
            return Err("invalid proof scope".into());
        }
        Ok(Self {
            living: living as u8,
            equality_ordinal: 1,
            operand: None,
        })
    }
    pub(super) fn operand(self, operand: Operand) -> Self {
        Self {
            operand: Some(operand),
            ..self
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
#[repr(usize)]
pub(super) enum ProofPhase {
    CanonicalExport,
    RewindAndBind,
    StrictPass1,
    TypedPass2DecodeSpool,
    SpoolFinalize,
    GenericStateAssembly,
    HistoryCleanup,
    ExportedStateDigestValidation,
    OperandSnapshotCleanup,
    CompleteGenericByteEofEquality,
    ProductionStateDigestValidation,
    OperandACleanup,
    OperandBCleanup,
    RootSnapshotExport,
    RootStandaloneRestore,
    RootSnapshotCleanup,
    StateEqual,
    RestoredDisposal,
}
const PHASES: usize = 18;
#[derive(Clone, Copy, Default)]
struct PhaseTotal {
    calls: u64,
    ns: u64,
    bytes: u64,
}

enum EventSink {
    File(fs::File),
    #[cfg(test)]
    Memory(TestSink),
}
impl EventSink {
    fn publish(&mut self, line: &[u8]) -> io::Result<()> {
        match self {
            Self::File(file) => {
                file.write_all(line)?;
                file.flush()?;
                file.sync_all()
            }
            #[cfg(test)]
            Self::Memory(sink) => {
                if sink.fail_record == Some(sink.records) {
                    return Err(bad("injected profile boundary cancellation"));
                }
                sink.records += 1;
                sink.write_all(line)?;
                sink.flush()?;
                sink.sync()
            }
        }
    }
}
/// Retained evidence: intentionally no directory-removing Drop implementation.
/// A publication failure poisons the collection; it can never silently fall back.
pub(super) struct GenericProofProfile {
    sink: EventSink,
    origin: Instant,
    seq: u64,
    bytes: u64,
    totals: [PhaseTotal; PHASES],
    failed: bool,
    sink_failed: bool,
    finished: bool,
    #[cfg(test)]
    clock_reads: std::rc::Rc<std::cell::Cell<u64>>,
    #[cfg(test)]
    after_phase: Option<(ProofPhase, Box<dyn FnOnce()>)>,
    #[cfg(test)]
    owned_directories: Vec<std::path::PathBuf>,
    #[cfg(test)]
    owned_files: Vec<(i32, u64, u64)>,
}
impl GenericProofProfile {
    #[cfg(test)]
    pub(super) fn memory(
        fail_record: Option<usize>,
    ) -> (Self, std::rc::Rc<std::cell::RefCell<Vec<u8>>>) {
        let sink = TestSink {
            fail_record,
            ..Default::default()
        };
        let data = sink.data.clone();
        (Self::new(EventSink::Memory(sink)).unwrap(), data)
    }
    #[cfg(test)]
    pub(super) fn test_after_phase(&mut self, phase: ProofPhase, action: impl FnOnce() + 'static) {
        self.after_phase = Some((phase, Box::new(action)));
    }
    #[cfg(test)]
    pub(super) fn track_directory(&mut self, path: &Path) {
        if matches!(self.sink, EventSink::Memory(_)) {
            self.owned_directories.push(path.to_owned());
        }
    }
    #[cfg(test)]
    pub(super) fn track_file(&mut self, file: &fs::File) {
        use std::os::{fd::AsRawFd, unix::fs::MetadataExt};
        if !matches!(self.sink, EventSink::Memory(_)) {
            return;
        }
        let metadata = file.metadata().unwrap();
        self.owned_files
            .push((file.as_raw_fd(), metadata.dev(), metadata.ino()));
    }
    #[cfg(test)]
    pub(super) fn assert_scratch_released(&self) {
        use std::os::unix::fs::MetadataExt;
        for path in &self.owned_directories {
            assert_eq!(
                fs::symlink_metadata(path).unwrap_err().kind(),
                io::ErrorKind::NotFound,
                "{path:?}"
            );
        }
        for (fd, dev, ino) in &self.owned_files {
            match fs::metadata(format!("/proc/self/fd/{fd}")) {
                Ok(metadata) => assert_ne!(
                    (metadata.dev(), metadata.ino()),
                    (*dev, *ino),
                    "owned scratch descriptor still open"
                ),
                Err(error) => assert_eq!(error.kind(), io::ErrorKind::NotFound),
            }
        }
    }
    pub(super) fn fail(&mut self) {
        self.failed = true;
    }
    pub(super) fn create(base: &Path) -> Result<Self, String> {
        // components() normalizes interior '.', so validate raw Unix segments first.
        if !base.is_absolute()
            || base
                .as_os_str()
                .as_bytes()
                .split(|b| *b == b'/')
                .any(|s| s == b"." || s == b"..")
            || base
                .components()
                .any(|c| !matches!(c, Component::RootDir | Component::Normal(_)))
        {
            return Err("proof profile needs plain absolute components".into());
        }
        for ancestor in base.ancestors() {
            let m = fs::symlink_metadata(ancestor).map_err(|e| e.to_string())?;
            if !m.is_dir() || m.file_type().is_symlink() {
                return Err("proof profile ancestry must be plain directories".into());
            }
        }
        if fs::symlink_metadata(base)
            .map_err(|e| e.to_string())?
            .permissions()
            .mode()
            & 0o7777
            != 0o700
        {
            return Err("proof profile base must be mode0700".into());
        }
        let child = base.join("generic-proof-phase-v1");
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&child)
            .map_err(|e| e.to_string())?;
        if fs::symlink_metadata(&child)
            .map_err(|e| e.to_string())?
            .permissions()
            .mode()
            & 0o7777
            != 0o700
        {
            return Err("proof profile directory must be mode0700".into());
        }
        let file = private_file(&child.join("events.ndjson")).map_err(|e| e.to_string())?;
        let m = file.metadata().map_err(|e| e.to_string())?;
        if !m.is_file() || m.permissions().mode() & 0o7777 != 0o600 {
            return Err("proof profile file must be regular mode0600".into());
        }
        Self::new(EventSink::File(file))
    }
    fn new(sink: EventSink) -> Result<Self, String> {
        let mut profile = Self {
            sink,
            origin: Instant::now(),
            seq: 0,
            bytes: 0,
            totals: [PhaseTotal::default(); PHASES],
            failed: false,
            sink_failed: false,
            finished: false,
            #[cfg(test)]
            clock_reads: Default::default(),
            #[cfg(test)]
            after_phase: None,
            #[cfg(test)]
            owned_directories: Vec::new(),
            #[cfg(test)]
            owned_files: Vec::new(),
        };
        profile.emit(serde_json::json!({"schema":PROFILE_SCHEMA,"kind":"header","seq":0,
            "build_id":super::BUILD_ID,"collection_scope":"root_proofs_only","instrumentation":true}))?;
        Ok(profile)
    }
    pub(super) fn now(&self) -> Instant {
        #[cfg(test)]
        if matches!(self.sink, EventSink::Memory(_)) {
            self.clock_reads.set(self.clock_reads.get() + 1);
        }
        Instant::now()
    }
    fn at(&self, time: Instant) -> Result<u64, String> {
        u64::try_from(time.duration_since(self.origin).as_nanos())
            .map_err(|_| "proof profile time overflow".into())
    }
    fn emit(&mut self, event: Value) -> Result<(), String> {
        if self.sink_failed || self.finished {
            return Err("proof profile sink is poisoned or closed".into());
        }
        let result = (|| {
            let mut line = serde_json::to_vec(&event).map_err(|e| e.to_string())?;
            line.push(b'\n');
            let next = self
                .seq
                .checked_add(1)
                .ok_or("proof profile sequence overflow")?;
            let bytes = self
                .bytes
                .checked_add(line.len() as u64)
                .ok_or("proof profile size overflow")?;
            if next > MAX_RECORDS || line.len() > MAX_LINE || bytes > MAX_BYTES {
                return Err("proof profile bound exceeded".into());
            }
            self.sink.publish(&line).map_err(|e| e.to_string())?;
            self.seq = next;
            self.bytes = bytes;
            Ok(())
        })();
        if result.is_err() {
            self.sink_failed = true;
            self.failed = true;
        }
        result
    }
    pub(super) fn begin(
        &mut self,
        scope: ProofScope,
        phase: ProofPhase,
    ) -> Result<Instant, String> {
        if self.failed {
            return Err("proof profile collection failed".into());
        }
        let at = self.at(self.now())?;
        self.emit(
            serde_json::json!({"schema":PROFILE_SCHEMA,"kind":"begin","seq":self.seq,
            "scope":scope,"phase":phase,"at_ns":at,"duration_ns":null,"outcome":null,"bytes":null}),
        )?;
        // Start after publication, so phase duration excludes its begin I/O.
        Ok(self.now())
    }
    pub(super) fn end(
        &mut self,
        scope: ProofScope,
        phase: ProofPhase,
        start: Instant,
        end: Instant,
        ok: bool,
        bytes: Option<u64>,
    ) -> Result<(), String> {
        let result = (|| {
            let ns = u64::try_from(end.duration_since(start).as_nanos())
                .map_err(|_| "proof profile duration overflow")?;
            if ok && !self.failed {
                let total = &mut self.totals[phase as usize];
                let next = PhaseTotal {
                    calls: total
                        .calls
                        .checked_add(1)
                        .ok_or("proof profile calls overflow")?,
                    ns: total
                        .ns
                        .checked_add(ns)
                        .ok_or("proof profile total time overflow")?,
                    bytes: total
                        .bytes
                        .checked_add(bytes.unwrap_or(0))
                        .ok_or("proof profile total bytes overflow")?,
                };
                *total = next;
            }
            let ok = ok && !self.failed;
            self.emit(
                serde_json::json!({"schema":PROFILE_SCHEMA,"kind":"end","seq":self.seq,
                "scope":scope,"phase":phase,"at_ns":self.at(end)?,"duration_ns":ns,
                "outcome":if ok {"ok"} else {"error"},"bytes":if ok {bytes} else {None}}),
            )?;
            if !ok {
                self.failed = true;
            }
            #[cfg(test)]
            if ok
                && self
                    .after_phase
                    .as_ref()
                    .is_some_and(|(target, _)| *target == phase)
            {
                let (_, action) = self.after_phase.take().unwrap();
                action();
            }
            Ok(())
        })();
        if result.is_err() {
            self.failed = true;
        }
        result
    }
    pub(super) fn finish(&mut self, complete: bool) -> Result<(), String> {
        let at = self.at(self.now())?;
        let complete = complete && !self.failed;
        self.emit(serde_json::json!({"schema":PROFILE_SCHEMA,"kind":"terminal","seq":self.seq,
            "at_ns":at,"collection_outcome":if complete {"root_collection_complete"} else {"error"}}))?;
        self.finished = true;
        if complete {
            Ok(())
        } else {
            Err("proof profile collection incomplete".into())
        }
    }
}
// None executes the original operation without reading any clock or collector.
pub(super) fn measured<T>(
    profile: &mut Option<&mut GenericProofProfile>,
    scope: ProofScope,
    phase: ProofPhase,
    operation: impl FnOnce(&mut Option<&mut GenericProofProfile>) -> Result<T, String>,
    bytes: impl FnOnce(&T) -> Option<u64>,
) -> Result<T, String> {
    let start = profile
        .as_deref_mut()
        .map(|p| p.begin(scope, phase))
        .transpose()?;
    let result = operation(profile);
    if let (Some(p), Some(start)) = (profile.as_deref_mut(), start) {
        let end = p.now(); // Read immediately after the operation, before publication.
        let count = result.as_ref().ok().and_then(bytes);
        if let Err(observer) = p.end(scope, phase, start, end, result.is_ok(), count) {
            return Err(match result {
                Ok(_) => observer,
                Err(error) => format!("{error}; proof profile: {observer}"),
            });
        }
    }
    result
}

#[cfg(test)]
#[derive(Default)]
struct TestSink {
    fail_record: Option<usize>,
    records: usize,
    data: std::rc::Rc<std::cell::RefCell<Vec<u8>>>,
    fail_after: Option<usize>,
    fail_flush: bool,
    fail_sync: bool,
    fragment: Option<usize>,
    interrupt: bool,
}
#[cfg(test)]
impl Write for TestSink {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        if std::mem::take(&mut self.interrupt) {
            return Err(io::ErrorKind::Interrupted.into());
        }
        let used = self.data.borrow().len();
        if self.fail_after.is_some_and(|n| used >= n) {
            return Err(bad("injected profile write failure"));
        }
        let n = data
            .len()
            .min(self.fragment.unwrap_or(usize::MAX))
            .min(self.fail_after.map_or(usize::MAX, |n| n - used));
        self.data.borrow_mut().extend_from_slice(&data[..n]);
        Ok(n)
    }
    fn flush(&mut self) -> io::Result<()> {
        if self.fail_flush {
            Err(bad("injected profile flush failure"))
        } else {
            Ok(())
        }
    }
}
#[cfg(test)]
impl TestSink {
    fn sync(&mut self) -> io::Result<()> {
        if self.fail_sync {
            Err(bad("injected profile sync failure"))
        } else {
            Ok(())
        }
    }
}

const BUFFER: usize = 64 * 1024;
fn bad(reason: &str) -> io::Error {
    io::Error::other(reason)
}

/// Counts and hashes only valid accepted bytes. Any failed operation poisons
/// finalization, including a flush error after every output byte was accepted.
struct AcceptedWriter<W> {
    sink: W,
    bytes: u64,
    hash: Sha256Sink,
    failed: bool,
}
impl<W: Write> AcceptedWriter<W> {
    fn new(sink: W) -> Self {
        Self {
            sink,
            bytes: 0,
            hash: Sha256Sink::new(),
            failed: false,
        }
    }
    fn finish(mut self) -> io::Result<(W, RawProof)> {
        self.flush()?;
        Ok((
            self.sink,
            RawProof {
                bytes: self.bytes,
                sha256: self.hash.finalize_hex(),
            },
        ))
    }
}
impl<W: Write> Write for AcceptedWriter<W> {
    fn write(&mut self, input: &[u8]) -> io::Result<usize> {
        if self.failed {
            return Err(bad("generic writer was poisoned"));
        }
        let result = (|| {
            let n = loop {
                match self.sink.write(input) {
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    other => break other?,
                }
            };
            if n > input.len() || (n == 0 && !input.is_empty()) {
                return Err(bad("invalid generic accepted write count"));
            }
            let next = self
                .bytes
                .checked_add(u64::try_from(n).map_err(io::Error::other)?)
                .ok_or_else(|| bad("generic write count overflow"))?;
            self.hash.update(&input[..n]);
            self.bytes = next;
            Ok(n)
        })();
        if result.is_err() {
            self.failed = true;
        }
        result
    }
    fn flush(&mut self) -> io::Result<()> {
        if self.failed {
            return Err(bad("generic writer was poisoned"));
        }
        let result = loop {
            match self.sink.flush() {
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                other => break other,
            }
        };
        if result.is_err() {
            self.failed = true;
        }
        result
    }
}
#[derive(Debug, PartialEq, Eq)]
struct RawProof {
    bytes: u64,
    sha256: String,
}

/// Binds BOTH complete strict-reader passes to the preceding export receipt.
/// Only the strict reader's one rewind is permitted; Current(0) is observational.
struct BoundReader<'a, R> {
    profile: Option<&'a mut GenericProofProfile>,
    scope: ProofScope,
    pass_start: Option<Instant>,
    source: R,
    expected: RawProof,
    bytes: u64,
    hash: Sha256Sink,
    completed: usize,
    eof: bool,
    rewound: bool,
    failed: bool,
}
impl<R: Read + Seek> BoundReader<'_, R> {
    fn new(mut source: R, expected: RawProof) -> io::Result<Self> {
        if source.stream_position()? != 0 {
            return Err(bad("snapshot proof reader must start at zero"));
        }
        Ok(Self {
            profile: None,
            scope: ProofScope::root(4).map_err(|e| bad(&e))?,
            pass_start: None,
            source,
            expected,
            bytes: 0,
            hash: Sha256Sink::new(),
            completed: 0,
            eof: false,
            rewound: false,
            failed: false,
        })
    }
    fn finish(self) -> io::Result<R> {
        if self.failed || self.completed != 2 || !self.eof || !self.rewound {
            return Err(bad("incomplete receipt-bound snapshot passes"));
        }
        Ok(self.source)
    }
}
impl<R: Read + Seek> Read for BoundReader<'_, R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if self.failed {
            return Err(bad("snapshot proof reader was poisoned"));
        }
        if buffer.is_empty() {
            return Ok(0);
        }
        let result = (|| {
            let n = loop {
                match self.source.read(buffer) {
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    other => break other?,
                }
            };
            if n > buffer.len() {
                return Err(bad("invalid snapshot read count"));
            }
            if self.eof {
                return if n == 0 {
                    Ok(0)
                } else {
                    Err(bad("snapshot bytes appeared after complete EOF"))
                };
            }
            self.bytes = self
                .bytes
                .checked_add(u64::try_from(n).map_err(io::Error::other)?)
                .ok_or_else(|| bad("snapshot read count overflow"))?;
            self.hash.update(&buffer[..n]);
            if n == 0 {
                let hash = std::mem::take(&mut self.hash).finalize_hex();
                if self.bytes != self.expected.bytes || hash != self.expected.sha256 {
                    return Err(bad("snapshot read differs from successful export receipt"));
                }
                self.completed += 1;
                self.eof = true;
            }
            Ok(n)
        })();
        if result.is_err() {
            self.failed = true;
        }
        result
    }
}
impl<R: Read + Seek> Seek for BoundReader<'_, R> {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        if self.failed {
            return Err(bad("snapshot proof reader was poisoned"));
        }
        let result = (|| match position {
            SeekFrom::Current(0) => {
                let actual = self.source.stream_position()?;
                if actual != self.bytes {
                    return Err(bad("snapshot reader position changed"));
                }
                Ok(actual)
            }
            SeekFrom::Start(0) if self.eof && self.completed == 1 && !self.rewound => {
                // strict_read_seekable reaches this only after ALL strict pass1
                // parser/end checks. Raw EOF alone never publishes pass1 success.
                if let (Some(p), Some(start)) =
                    (self.profile.as_deref_mut(), self.pass_start.take())
                {
                    let end = p.now();
                    p.end(
                        self.scope,
                        ProofPhase::StrictPass1,
                        start,
                        end,
                        true,
                        Some(self.expected.bytes),
                    )
                    .map_err(|e| bad(&e))?;
                }
                if self.source.seek(SeekFrom::Start(0))? != 0 {
                    return Err(bad("snapshot rewind failed"));
                }
                if let Some(p) = self.profile.as_deref_mut() {
                    self.pass_start = Some(
                        p.begin(self.scope, ProofPhase::TypedPass2DecodeSpool)
                            .map_err(|e| bad(&e))?,
                    );
                }
                self.bytes = 0;
                self.eof = false;
                self.rewound = true;
                Ok(0)
            }
            _ => Err(bad("unexpected snapshot proof seek")),
        })();
        if result.is_err() {
            self.failed = true;
        }
        result
    }
}

#[derive(Debug)]
struct Range {
    start: u64,
    length: u64,
}
struct Knowledge {
    fields: BTreeMap<String, Value>,
    history: Range,
}
struct State {
    fields: BTreeMap<String, Value>,
    visibility: Vec<Knowledge>,
}
struct SpoolingSnapshot {
    owner: GenericStateProof,
    spool: BufWriter<AcceptedWriter<fs::File>>,
    state: State,
}
struct StateSeed<'a> {
    spool: &'a mut BufWriter<AcceptedWriter<fs::File>>,
}
struct VisibilitySeed<'a> {
    spool: &'a mut BufWriter<AcceptedWriter<fs::File>>,
}
struct KnowledgeSeed<'a> {
    spool: &'a mut BufWriter<AcceptedWriter<fs::File>>,
}
struct HistorySeed<'a> {
    spool: &'a mut BufWriter<AcceptedWriter<fs::File>>,
}

impl<'de> DeserializeSeed<'de> for HistorySeed<'_> {
    type Value = Range;
    fn deserialize<D: de::Deserializer<'de>>(self, d: D) -> Result<Range, D::Error> {
        struct V<'a>(&'a mut BufWriter<AcceptedWriter<fs::File>>);
        impl<'de> Visitor<'de> for V<'_> {
            type Value = Range;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a complete history array")
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<Range, A::Error> {
                self.0.flush().map_err(de::Error::custom)?;
                let start = self.0.get_ref().bytes;
                self.0.write_all(b"[").map_err(de::Error::custom)?;
                let mut first = true;
                // Value is intentional: this is the independent generic oracle,
                // not a typed StoredPermittedView or its canonical memo.
                while let Some(view) = a.next_element::<Value>()? {
                    if !first {
                        self.0.write_all(b",").map_err(de::Error::custom)?;
                    }
                    serde_json::to_writer(&mut *self.0, &view).map_err(de::Error::custom)?;
                    first = false;
                }
                self.0.write_all(b"]").map_err(de::Error::custom)?;
                self.0.flush().map_err(de::Error::custom)?;
                let length = self
                    .0
                    .get_ref()
                    .bytes
                    .checked_sub(start)
                    .ok_or_else(|| de::Error::custom("history spool count regressed"))?;
                Ok(Range { start, length })
            }
        }
        d.deserialize_seq(V(self.spool))
    }
}
impl<'de> DeserializeSeed<'de> for KnowledgeSeed<'_> {
    type Value = Knowledge;
    fn deserialize<D: de::Deserializer<'de>>(self, d: D) -> Result<Knowledge, D::Error> {
        struct V<'a>(&'a mut BufWriter<AcceptedWriter<fs::File>>);
        impl<'de> Visitor<'de> for V<'_> {
            type Value = Knowledge;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a Knowledge object with one history")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<Knowledge, A::Error> {
                let mut fields = BTreeMap::new();
                let mut history = None;
                while let Some(key) = a.next_key::<String>()? {
                    if fields.contains_key(&key) {
                        return Err(de::Error::custom("duplicate Knowledge field"));
                    }
                    let value = if key == "history" {
                        history = Some(a.next_value_seed(HistorySeed { spool: self.0 })?);
                        Value::Null
                    } else {
                        a.next_value::<Value>()?
                    };
                    fields.insert(key, value);
                }
                Ok(Knowledge {
                    fields,
                    history: history
                        .ok_or_else(|| de::Error::custom("missing Knowledge history"))?,
                })
            }
        }
        d.deserialize_map(V(self.spool))
    }
}
impl<'de> DeserializeSeed<'de> for VisibilitySeed<'_> {
    type Value = Vec<Knowledge>;
    fn deserialize<D: de::Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
        struct V<'a>(&'a mut BufWriter<AcceptedWriter<fs::File>>);
        impl<'de> Visitor<'de> for V<'_> {
            type Value = Vec<Knowledge>;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("the complete visibility array")
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<Self::Value, A::Error> {
                let mut result = Vec::new();
                while let Some(row) = a.next_element_seed(KnowledgeSeed { spool: self.0 })? {
                    result.push(row);
                }
                Ok(result)
            }
        }
        d.deserialize_seq(V(self.spool))
    }
}
impl<'de> DeserializeSeed<'de> for StateSeed<'_> {
    type Value = State;
    fn deserialize<D: de::Deserializer<'de>>(self, d: D) -> Result<State, D::Error> {
        struct V<'a>(&'a mut BufWriter<AcceptedWriter<fs::File>>);
        impl<'de> Visitor<'de> for V<'_> {
            type Value = State;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a complete State object")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<State, A::Error> {
                let mut fields = BTreeMap::new();
                let mut visibility = None;
                while let Some(key) = a.next_key::<String>()? {
                    if fields.contains_key(&key) {
                        return Err(de::Error::custom("duplicate State field"));
                    }
                    let value = if key == "visibility" {
                        visibility = Some(a.next_value_seed(VisibilitySeed { spool: self.0 })?);
                        Value::Null
                    } else {
                        a.next_value::<Value>()?
                    };
                    fields.insert(key, value);
                }
                Ok(State {
                    fields,
                    visibility: visibility
                        .ok_or_else(|| de::Error::custom("missing State visibility"))?,
                })
            }
        }
        d.deserialize_map(V(self.spool))
    }
}
impl<'de> Deserialize<'de> for SpoolingSnapshot {
    fn deserialize<D: de::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = SpoolingSnapshot;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("one snapshot object containing State")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<Self::Value, A::Error> {
                let owner = GenericStateProof::create().map_err(de::Error::custom)?;
                let file = private_file(&owner.directory.join("histories.json"))
                    .map_err(de::Error::custom)?;
                let mut spool = BufWriter::with_capacity(BUFFER, AcceptedWriter::new(file));
                let mut state = None;
                while let Some(key) = a.next_key::<String>()? {
                    if key == "state" {
                        if state.is_some() {
                            return Err(de::Error::custom("duplicate snapshot State"));
                        }
                        state = Some(a.next_value_seed(StateSeed { spool: &mut spool })?);
                    } else {
                        // Uniqueness was checked over the ENTIRE document by
                        // pass1; raw receipt binding also covers this trace.
                        a.next_value::<IgnoredAny>()?;
                    }
                }
                Ok(SpoolingSnapshot {
                    owner,
                    spool,
                    state: state.ok_or_else(|| de::Error::custom("missing snapshot State"))?,
                })
            }
        }
        d.deserialize_map(V)
    }
}
fn private_file(path: &std::path::Path) -> io::Result<fs::File> {
    fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
}
fn generic(writer: &mut impl Write, value: &impl serde::Serialize) -> io::Result<()> {
    serde_json::to_writer(writer, value).map_err(io::Error::other)
}
fn copy_range(
    reader: &mut impl Read,
    writer: &mut impl Write,
    length: u64,
    hash: &mut Sha256Sink,
) -> io::Result<()> {
    let mut remaining = length;
    let mut buffer = [0u8; BUFFER];
    while remaining != 0 {
        let limit = usize::try_from(remaining.min(BUFFER as u64)).map_err(io::Error::other)?;
        let n = loop {
            match reader.read(&mut buffer[..limit]) {
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                other => break other?,
            }
        };
        if n == 0 || n > limit {
            return Err(bad("invalid or truncated history spool read"));
        }
        writer.write_all(&buffer[..n])?;
        hash.update(&buffer[..n]);
        remaining -= n as u64;
    }
    Ok(())
}
fn write_state(
    state: &State,
    spool: &mut impl Read,
    writer: &mut impl Write,
    expected_spool: &RawProof,
) -> io::Result<()> {
    let mut copied = 0u64;
    let mut spool_hash = Sha256Sink::new();
    writer.write_all(b"{")?;
    for (index, (key, value)) in state.fields.iter().enumerate() {
        if index != 0 {
            writer.write_all(b",")?;
        }
        generic(writer, key)?;
        writer.write_all(b":")?;
        if key != "visibility" {
            generic(writer, value)?;
            continue;
        }
        writer.write_all(b"[")?;
        for (i, knowledge) in state.visibility.iter().enumerate() {
            if i != 0 {
                writer.write_all(b",")?;
            }
            writer.write_all(b"{")?;
            for (j, (key, value)) in knowledge.fields.iter().enumerate() {
                if j != 0 {
                    writer.write_all(b",")?;
                }
                generic(writer, key)?;
                writer.write_all(b":")?;
                if key == "history" {
                    if knowledge.history.start != copied {
                        return Err(bad("history range is not contiguous and ordered"));
                    }
                    copy_range(spool, writer, knowledge.history.length, &mut spool_hash)?;
                    copied = copied
                        .checked_add(knowledge.history.length)
                        .ok_or_else(|| bad("history range overflow"))?;
                } else {
                    generic(writer, value)?;
                }
            }
            writer.write_all(b"}")?;
        }
        writer.write_all(b"]")?;
    }
    writer.write_all(b"}")?;
    let mut tail = [0u8; 1];
    let n = loop {
        match spool.read(&mut tail) {
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            other => break other?,
        }
    };
    if n != 0
        || copied != expected_spool.bytes
        || spool_hash.finalize_hex() != expected_spool.sha256
    {
        return Err(bad("complete history spool identity/EOF differs"));
    }
    Ok(())
}
impl SpoolingSnapshot {
    fn finish(self) -> Result<GenericStateProof, String> {
        self.finish_with_profile(ProofScope::root(4)?, None)
    }
    fn finish_with_profile(
        self,
        scope: ProofScope,
        mut profile: Option<&mut GenericProofProfile>,
    ) -> Result<GenericStateProof, String> {
        let Self {
            mut owner,
            mut spool,
            state,
        } = self;
        #[cfg(test)]
        if let Some(p) = profile.as_deref_mut() {
            p.track_directory(&owner.directory);
            p.track_file(&spool.get_ref().sink);
        }
        let (mut history_file, history_proof) = measured(
            &mut profile,
            scope,
            ProofPhase::SpoolFinalize,
            |_| {
                spool.flush().map_err(|e| e.to_string())?;
                let checked = spool.into_inner().map_err(|e| e.to_string())?;
                let (mut history_file, history_proof) =
                    checked.finish().map_err(|e| e.to_string())?;
                if history_file.metadata().map_err(|e| e.to_string())?.len() != history_proof.bytes
                {
                    return Err("generic history spool length differs".into());
                }
                history_file
                    .seek(SeekFrom::Start(0))
                    .map_err(|e| e.to_string())?;
                Ok((history_file, history_proof))
            },
            |(_, proof)| Some(proof.bytes),
        )?;
        let (file, proof) = measured(
            &mut profile,
            scope,
            ProofPhase::GenericStateAssembly,
            |_| {
                let file = private_file(&owner.path).map_err(|e| e.to_string())?;
                let mut output = BufWriter::with_capacity(BUFFER, AcceptedWriter::new(file));
                write_state(&state, &mut history_file, &mut output, &history_proof)
                    .map_err(|e| e.to_string())?;
                output.flush().map_err(|e| e.to_string())?;
                let checked = output.into_inner().map_err(|e| e.to_string())?;
                let (file, proof) = checked.finish().map_err(|e| e.to_string())?;
                if file.metadata().map_err(|e| e.to_string())?.len() != proof.bytes {
                    return Err("generic State file length differs".into());
                }
                Ok((file, proof))
            },
            |(_, proof)| Some(proof.bytes),
        )?;
        measured(
            &mut profile,
            scope,
            ProofPhase::HistoryCleanup,
            |_| {
                drop(file);
                drop(history_file);
                fs::remove_file(owner.directory.join("histories.json"))
                    .map_err(|e| e.to_string())?;
                owner.bytes = proof.bytes;
                owner.sha256 = proof.sha256;
                Ok(owner)
            },
            |owner| Some(owner.bytes),
        )
    }
}
fn decode<R: Read + Seek>(reader: R, proof: RawProof) -> Result<SpoolingSnapshot, String> {
    decode_with_profile(reader, proof, ProofScope::root(4)?, None)
}
fn decode_with_profile<R: Read + Seek>(
    reader: R,
    proof: RawProof,
    scope: ProofScope,
    mut profile: Option<&mut GenericProofProfile>,
) -> Result<SpoolingSnapshot, String> {
    let result = (|| {
        let bound = BoundReader::new(reader, proof).map_err(|e| e.to_string())?;
        decode_bound_with_profile(bound, scope, profile.as_deref_mut())
    })();
    if result.is_err()
        && let Some(p) = profile
    {
        p.fail();
    }
    result
}
fn decode_bound_with_profile<R: Read + Seek>(
    bound: BoundReader<'_, R>,
    scope: ProofScope,
    mut profile: Option<&mut GenericProofProfile>,
) -> Result<SpoolingSnapshot, String> {
    // Move only the untouched binding into the borrowed adapter, then release
    // that borrow before later finalization. No Serde visitor observer is used.
    let mut bound = BoundReader {
        source: bound.source,
        expected: bound.expected,
        bytes: bound.bytes,
        hash: bound.hash,
        completed: bound.completed,
        eof: bound.eof,
        rewound: bound.rewound,
        failed: bound.failed,
        scope,
        pass_start: profile
            .as_deref_mut()
            .map(|p| p.begin(scope, ProofPhase::StrictPass1))
            .transpose()?,
        profile: profile.as_deref_mut(),
    };
    let result = strict_read_seekable(&mut bound);
    let phase = if bound.rewound {
        ProofPhase::TypedPass2DecodeSpool
    } else {
        ProofPhase::StrictPass1
    };
    let start = bound.pass_start.take();
    let bytes = bound.expected.bytes;
    // finish() checks both exact complete passes and poison, even on typed
    // decoder success. No typed success receipt can precede this check.
    let result =
        result.and_then(|parsed| bound.finish().map(|_| parsed).map_err(|e| e.to_string()));
    if let Some(p) = profile {
        let end = p.now();
        if let Some(start) = start {
            p.end(
                scope,
                phase,
                start,
                end,
                result.is_ok(),
                result.as_ref().ok().map(|_| bytes),
            )?;
        }
        if result.is_err() {
            p.failed = true;
        }
    }
    result
}
pub(super) fn prove(game: &Game) -> Result<GenericStateProof, String> {
    prove_with_profile(game, ProofScope::root(4)?, None)
}
pub(super) fn prove_with_profile(
    game: &Game,
    scope: ProofScope,
    mut profile: Option<&mut GenericProofProfile>,
) -> Result<GenericStateProof, String> {
    let result = (|| {
        let mut snapshot = GenericStateProof::create()?;
        let mut file = private_file(&snapshot.path).map_err(|e| e.to_string())?;
        #[cfg(test)]
        if let Some(p) = profile.as_deref_mut() {
            p.track_directory(&snapshot.directory);
            p.track_file(&file);
        }
        let receipt = measured(
            &mut profile,
            scope,
            ProofPhase::CanonicalExport,
            |_| {
                let receipt = game.write_snapshot(&mut file)?;
                if file.metadata().map_err(|e| e.to_string())?.len() != receipt.bytes {
                    return Err("snapshot export length differs".into());
                }
                Ok(receipt)
            },
            |receipt| Some(receipt.bytes),
        )?;
        let expected_state_sha256 = receipt.state_sha256.clone();
        let bound = measured(
            &mut profile,
            scope,
            ProofPhase::RewindAndBind,
            |_| {
                file.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
                snapshot.bytes = receipt.bytes;
                snapshot.sha256 = receipt.snapshot_sha256;
                BoundReader::new(
                    file,
                    RawProof {
                        bytes: snapshot.bytes,
                        sha256: snapshot.sha256.clone(),
                    },
                )
                .map_err(|e| e.to_string())
            },
            |_| None,
        )?;
        let parsed = decode_bound_with_profile(bound, scope, profile.as_deref_mut())?;
        let proof = parsed.finish_with_profile(scope, profile.as_deref_mut())?;
        measured(
            &mut profile,
            scope,
            ProofPhase::ExportedStateDigestValidation,
            |_| {
                if proof.sha256 != expected_state_sha256 {
                    return Err("generic State differs from snapshot State digest".into());
                }
                Ok(())
            },
            |_| None,
        )?;
        measured(
            &mut profile,
            scope,
            ProofPhase::OperandSnapshotCleanup,
            |_| snapshot.remove(),
            |_| None,
        )?;
        Ok(proof)
    })();
    if result.is_err()
        && let Some(p) = profile
    {
        p.fail();
    }
    result
}

#[cfg(test)]
#[path = "generic_state_spool_tests.rs"]
mod tests;
