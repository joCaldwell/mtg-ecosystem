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
}
impl<R: Read> Equal<R> {
    fn new(reader: R) -> Self {
        Self {
            reader,
            hash: Sha256Sink::new(),
            bytes: 0,
            poisoned: false,
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
            let mut block = [0; BLOCK];
            while done < buf.len() {
                let size = (buf.len() - done).min(BLOCK);
                let n = read_retry(&mut self.reader, &mut block[..size])?;
                if n == 0 {
                    return Err(invalid("actual contains bytes beyond reference EOF"));
                }
                if block[..n] != buf[done..done + n] {
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
    let mut comparison = Equal::new(BufReader::with_capacity(BLOCK, file));
    actual(&mut comparison)?;
    comparison.flush().map_err(text)?;
    if comparison.finish()? != expected {
        return Err("complete comparison count/SHA mismatch".into());
    }
    Ok(())
}

#[derive(Debug)]
enum ReferenceError {
    Query(QueryRejection),
    Proof(String),
}
struct Reference {
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
    fn new(work: &Work, directory: &Directory) -> Result<Self, String> {
        let WorkData {
            query,
            task,
            units_done,
        } = &work.data;
        let Task::Search {
            builder,
            observation,
            history,
            decks,
        } = task
        else {
            return Err("reference requires Search".into());
        };
        if *units_done != 0 || !history.is_empty() || !decks.is_empty() || observation.is_some() {
            return Err("reference must start at lazy zero".into());
        }
        Ok(Self {
            query: query.clone(),
            builder: builder.clone(),
            observation: None,
            history: HistorySpool::new(directory).map_err(text)?,
            decks: vec![],
            units_done: 0,
            completed: false,
        })
    }
    fn unit(&mut self, game: &Game) -> Result<bool, ReferenceError> {
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
        let permitted = &game.state.visibility[viewer.0 as usize].history;
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
            task: OldTask::Search {
                builder: &self.builder,
                observation: &self.observation,
                history: &[],
                decks: &self.decks,
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
    let old = reference.data()?;
    let new = actual_data(actual)?;
    let Task::Search { history, .. } = &actual.task else {
        return Err("actual scratch not Search".into());
    };
    compare_bytes(
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
    )
}
pub(super) fn compare(game: &Game, query: Query) {
    let before = game.state.clone();
    let trace = game.trace.clone();
    let state_sha256 = game.state_digest().unwrap();
    for quantum in [1, 64, u64::MAX] {
        let directory = Directory::new().unwrap();
        let mut work = game.start_work(query.clone()).unwrap();
        let mut reference = Reference::new(&work, &directory).unwrap();
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
        scratch(&directory, &mut reference, &work.data).unwrap();
        let mut checkpoints = 0;
        loop {
            let expected = reference.resume(game, Budget::new(quantum));
            let actual = game.resume_work(work, Budget::new(quantum));
            match (actual, expected) {
                (QueryResult::Complete(QueryValue::SearchContext(context)), Ok(true)) => {
                    let old = reference.result(game).unwrap();
                    let new = actual_result(&context).unwrap();
                    compare_bytes(
                        &directory,
                        |out| {
                            replaced(&old, &["context", "permitted_history"], out, &mut |out| {
                                reference.history.write_array(out)
                            })
                        },
                        |out| {
                            replaced(&new, &["context", "permitted_history"], out, &mut |out| {
                                write_actual_history(&context.permitted_history, out)
                            })
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
                    scratch(&directory, &mut reference, &next.data).unwrap();
                    work = next;
                    // EXACT original sparse cold prefixes, not moved/shortened.
                    if (quantum == 1 && [1, 5, 31, 129].contains(&reference.units_done))
                        || (quantum == 64 && checkpoints == 0)
                    {
                        let saved = game.save_work(&work).unwrap();
                        let envelope = serde_json::json!({"work_version":15,"build_id":BUILD_ID,"catalog_digest":game.prepared.catalog_digest,"state_sha256":state_sha256,"data":reference.data().unwrap()});
                        compare_bytes(
                            &directory,
                            |out| {
                                replaced(&envelope, &["data", "task", "history"], out, &mut |out| {
                                    reference.history.write_array(out)
                                })
                            },
                            |out| out.write_all(saved.as_bytes()).map_err(text),
                        )
                        .unwrap();
                        let cold = game.restore_work(&saved).unwrap();
                        scratch(&directory, &mut reference, &cold.data).unwrap();
                        work = cold;
                        checkpoints += 1;
                    }
                }
                (_, Err(ReferenceError::Proof(error))) => {
                    panic!("complete Search reference proof I/O failed:{error}")
                }
                _ => panic!("optimized Search changed reference completion/rejection unit"),
            }
        }
        drop(reference);
        directory.cleanup().unwrap();
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
        Ok(last)
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
    let directory = Directory::new().map_err(text)?;
    let mut file = directory.create("snapshot.json").map_err(text)?;
    let written = {
        let mut writer = BufWriter::with_capacity(BLOCK, &mut file);
        let written = game.write_snapshot(&mut writer)?;
        writer.flush().map_err(text)?;
        written
    };
    if file.metadata().map_err(text)?.len() != written.bytes {
        return Err("written snapshot file length mismatch".into());
    }
    if file.seek(SeekFrom::Start(0)).map_err(text)? != 0 {
        return Err("snapshot file initial rewind mismatch".into());
    }
    let mut counted = ReceiptRead::new(&mut file).map_err(text)?;
    // This ordinary API performs the unchanged duplicate scan, typed decode,
    // admission, full State/invariant/trace and accepted-input replay proof.
    let restored = Game::restore_from_reader(game.prepared.as_ref().clone(), &mut counted)?;
    if counted.finish()? != (written.bytes, written.snapshot_sha256) {
        return Err("read snapshot bytes differ from writer receipt".into());
    }
    if file.metadata().map_err(text)?.len() != written.bytes {
        return Err("snapshot file changed length during restore".into());
    }
    if restored.state_digest()? != written.state_sha256 {
        return Err("restored full State differs from writer receipt".into());
    }
    drop(file);
    directory.cleanup().map_err(text)?;
    Ok(restored)
}
