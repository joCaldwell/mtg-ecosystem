//! Private canonical snapshot encoder.
//! String convenience and external Write APIs share this exact canonical
//! encoder; no whole-State Value or complete output buffer is required by Write.
use super::*;
use std::io::{BufWriter, Write};

const BUFFER: usize = 64 * 1024;

fn raw<W: Write>(writer: &mut W, bytes: &[u8]) -> Result<(), String> {
    writer.write_all(bytes).map_err(|error| error.to_string())
}
fn generic<W: Write, T: Serialize>(writer: &mut W, value: &T) -> Result<(), String> {
    // The same generic canonical ordering as contracts::canonical. Limit the
    // temporary Value to one metadata value or one original trace entry.
    let value = serde_json::to_value(value).map_err(|error| error.to_string())?;
    serde_json::to_writer(writer, &value).map_err(|error| error.to_string())
}
fn array<W: Write, T: Serialize>(writer: &mut W, history: &History<T>) -> Result<(), String> {
    raw(writer, b"[")?;
    for (index, entry) in history.iter().enumerate() {
        if index != 0 {
            raw(writer, b",")?;
        }
        generic(writer, entry)?;
    }
    raw(writer, b"]")
}
fn trace<W: Write>(writer: &mut W, trace: &TraceEvidence) -> Result<(), String> {
    // Exhaustive pattern: a new TraceEvidence field must be handled explicitly.
    // All four current fields are required, including null verification.
    let TraceEvidence {
        mode,
        verification,
        records,
        events,
    } = trace;
    raw(writer, b"{\"events\":")?;
    array(writer, events)?;
    raw(writer, b",\"mode\":")?;
    generic(writer, mode)?;
    raw(writer, b",\"records\":")?;
    array(writer, records)?;
    raw(writer, b",\"verification\":")?;
    generic(writer, verification)?;
    raw(writer, b"}")
}
fn unbuffered<W: Write>(snapshot: &SnapshotRef<'_>, writer: &mut W) -> Result<(), String> {
    // Exhaustive pattern: no SnapshotRef field may disappear. Object keys are
    // emitted in exactly serde Value's lexical order, not Rust field order.
    let SnapshotRef {
        snapshot_version,
        build_id,
        catalog_digest,
        profile_digest,
        state_sha256,
        state,
        trace: evidence,
    } = snapshot;
    raw(writer, b"{\"build_id\":")?;
    generic(writer, build_id)?;
    raw(writer, b",\"catalog_digest\":")?;
    generic(writer, catalog_digest)?;
    raw(writer, b",\"profile_digest\":")?;
    generic(writer, profile_digest)?;
    raw(writer, b",\"snapshot_version\":")?;
    generic(writer, snapshot_version)?;
    raw(writer, b",\"state\":")?;
    state_digest::write_state(state, writer)?;
    raw(writer, b",\"state_sha256\":")?;
    generic(writer, state_sha256)?;
    raw(writer, b",\"trace\":")?;
    trace(writer, evidence)?;
    raw(writer, b"}")
}
pub(super) fn write<W: Write>(snapshot: &SnapshotRef<'_>, sink: &mut W) -> Result<(), String> {
    let mut writer = BufWriter::with_capacity(BUFFER, sink);
    unbuffered(snapshot, &mut writer)?;
    // BufWriter's Drop cannot report flush failures. A successful result always
    // includes the underlying sink's flush; a failed sink may hold a prefix.
    writer.flush().map_err(|error| error.to_string())
}
pub(super) fn encode(snapshot: &SnapshotRef<'_>) -> Result<String, String> {
    let mut bytes = Vec::new();
    write(snapshot, &mut bytes)?;
    String::from_utf8(bytes).map_err(|error| error.to_string())
}

// Only accepted bytes count. An error may leave a prefix in the caller sink,
// but no successful receipt is returned until serialization AND flush succeed.
pub(super) struct WrittenBytes {
    pub(super) bytes: u64,
    pub(super) sha256: String,
}
struct CheckedWrite<W> {
    sink: W,
    bytes: u64,
    sha256: Sha256Sink,
}
impl<W: Write> Write for CheckedWrite<W> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let count = self.sink.write(bytes)?;
        if count > bytes.len() {
            return Err(std::io::Error::other(
                "writer returned an invalid byte count",
            ));
        }
        self.bytes = self
            .bytes
            .checked_add(u64::try_from(count).map_err(std::io::Error::other)?)
            .ok_or_else(|| std::io::Error::other("snapshot byte count overflow"))?;
        self.sha256.update(&bytes[..count]);
        Ok(count)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.sink.flush()
    }
}
pub(super) fn write_counted<W: Write>(
    snapshot: &SnapshotRef<'_>,
    sink: &mut W,
) -> Result<WrittenBytes, String> {
    let mut checked = CheckedWrite {
        sink,
        bytes: 0,
        sha256: Sha256Sink::new(),
    };
    write(snapshot, &mut checked)?;
    Ok(WrittenBytes {
        bytes: checked.bytes,
        sha256: checked.sha256.finalize_hex(),
    })
}
#[cfg(test)]
#[path = "streaming_snapshot_tests.rs"]
mod streaming_tests;

#[cfg(test)]
#[path = "snapshot_codec_tests.rs"]
mod tests;
