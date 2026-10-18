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
struct BoundReader<R> {
    source: R,
    expected: RawProof,
    bytes: u64,
    hash: Sha256Sink,
    completed: usize,
    eof: bool,
    rewound: bool,
    failed: bool,
}
impl<R: Read + Seek> BoundReader<R> {
    fn new(mut source: R, expected: RawProof) -> io::Result<Self> {
        if source.stream_position()? != 0 {
            return Err(bad("snapshot proof reader must start at zero"));
        }
        Ok(Self {
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
impl<R: Read + Seek> Read for BoundReader<R> {
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
impl<R: Read + Seek> Seek for BoundReader<R> {
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
                if self.source.seek(SeekFrom::Start(0))? != 0 {
                    return Err(bad("snapshot rewind failed"));
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
        let Self {
            mut owner,
            mut spool,
            state,
        } = self;
        spool.flush().map_err(|e| e.to_string())?;
        let checked = spool.into_inner().map_err(|e| e.to_string())?;
        let (mut history_file, history_proof) = checked.finish().map_err(|e| e.to_string())?;
        if history_file.metadata().map_err(|e| e.to_string())?.len() != history_proof.bytes {
            return Err("generic history spool length differs".into());
        }
        history_file
            .seek(SeekFrom::Start(0))
            .map_err(|e| e.to_string())?;
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
        drop(file);
        drop(history_file);
        fs::remove_file(owner.directory.join("histories.json")).map_err(|e| e.to_string())?;
        owner.bytes = proof.bytes;
        owner.sha256 = proof.sha256;
        Ok(owner)
    }
}
fn decode<R: Read + Seek>(reader: R, proof: RawProof) -> Result<SpoolingSnapshot, String> {
    let mut bound = BoundReader::new(reader, proof).map_err(|e| e.to_string())?;
    let parsed = strict_read_seekable(&mut bound)?;
    drop(bound.finish().map_err(|e| e.to_string())?);
    Ok(parsed)
}
pub(super) fn prove(game: &Game) -> Result<GenericStateProof, String> {
    let mut snapshot = GenericStateProof::create()?;
    let mut file = private_file(&snapshot.path).map_err(|e| e.to_string())?;
    let receipt = game.write_snapshot(&mut file)?;
    let expected_state_sha256 = receipt.state_sha256.clone();
    if file.metadata().map_err(|e| e.to_string())?.len() != receipt.bytes {
        return Err("snapshot export length differs".into());
    }
    file.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
    snapshot.bytes = receipt.bytes;
    snapshot.sha256 = receipt.snapshot_sha256;
    let parsed = decode(
        file,
        RawProof {
            bytes: snapshot.bytes,
            sha256: snapshot.sha256.clone(),
        },
    )?;
    let proof = parsed.finish()?;
    if proof.sha256 != expected_state_sha256 {
        return Err("generic State differs from snapshot State digest".into());
    }
    snapshot.remove()?;
    Ok(proof)
}

#[cfg(test)]
#[path = "generic_state_spool_tests.rs"]
mod tests;
