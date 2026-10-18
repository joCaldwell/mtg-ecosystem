//! Strict two-pass JSON decoding without retaining an input String or Value.
use super::*;
use std::io::{self, BufReader, Read, Seek, SeekFrom};

const BUFFER: usize = 64 * 1024;

#[derive(Debug, PartialEq, Eq)]
struct RawProof {
    bytes: u64,
    sha256: String,
}
struct CheckedRead<R> {
    source: R,
    bytes: u64,
    sha256: Sha256Sink,
}
impl<R: Read> Read for CheckedRead<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let count = loop {
            match self.source.read(buffer) {
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                result => break result?,
            }
        };
        if count > buffer.len() {
            return Err(io::Error::other("reader returned an invalid byte count"));
        }
        let count64 = u64::try_from(count).map_err(io::Error::other)?;
        self.bytes = self
            .bytes
            .checked_add(count64)
            .ok_or_else(|| io::Error::other("JSON byte count overflow"))?;
        self.sha256.update(&buffer[..count]);
        Ok(count)
    }
}
fn pass<T: de::DeserializeOwned>(source: &mut impl Read) -> Result<(T, RawProof), String> {
    let checked = CheckedRead {
        source,
        bytes: 0,
        sha256: Sha256Sink::new(),
    };
    let mut buffered = BufReader::with_capacity(BUFFER, checked);
    let value = {
        let mut decoder = serde_json::Deserializer::from_reader(&mut buffered);
        let value = T::deserialize(&mut decoder).map_err(|error| error.to_string())?;
        // Includes trailing whitespace to EOF. A second JSON value or any
        // nonwhitespace suffix is rejected rather than left for a caller.
        decoder.end().map_err(|error| error.to_string())?;
        value
    };
    if !buffered.buffer().is_empty() {
        return Err("JSON decoder left unverified buffered bytes".into());
    }
    let checked = buffered.into_inner();
    Ok((
        value,
        RawProof {
            bytes: checked.bytes,
            sha256: checked.sha256.finalize_hex(),
        },
    ))
}

/// Decode one strict JSON document from the current position through EOF.
/// Requires Seek to repeat the existing duplicate-key scan without buffering
/// the whole input. Both passes bind every raw byte, including whitespace;
/// changed seekable input is rejected before returning any typed value.
/// Success leaves the reader at EOF. Position on error is unspecified.
pub fn strict_read_seekable<T: de::DeserializeOwned, R: Read + Seek>(
    source: &mut R,
) -> Result<T, String> {
    let start = source
        .stream_position()
        .map_err(|error| error.to_string())?;
    let (_, scanned) = pass::<UniqueJson>(source)?;
    if source
        .seek(SeekFrom::Start(start))
        .map_err(|error| error.to_string())?
        != start
    {
        return Err("JSON reader failed to return to its original position".into());
    }
    let (typed, decoded) = pass::<T>(source)?;
    if scanned != decoded {
        return Err("JSON input changed between duplicate scan and typed decode".into());
    }
    Ok(typed)
}

#[cfg(test)]
#[path = "strict_io_tests.rs"]
mod tests;
