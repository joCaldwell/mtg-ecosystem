//! Compile an accepted archive into one pinned, compact bulk artifact.
use crate::coverage::{RecordAssessment, assess_record};
use mtg_contracts::{bulk_catalog::*, *};
use rustix::fs::{AtFlags, CWD, Dir, FileType, Mode, OFlags, fstat, openat, statat, unlinkat};
use serde::{Deserialize, Serialize};
use serde_json::{Value, value::RawValue};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::{self, BufRead, BufReader, BufWriter, Read, Write},
    os::unix::fs::DirBuilderExt,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Serialize)]
pub struct BulkBuildReport {
    pub manifest_sha256: String,
    pub manifest: BulkManifest,
}

/// Where a record's outcome was decided: archive scope/exclusion, the
/// archived reader result, compiler lowering, or IR validation after lowering.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutcomePhase {
    Scope,
    Read,
    Lower,
    Validate,
}

/// One `outcomes.jsonl` row per archive identity, in index order and joined
/// to the index by `oracle_id`, `source_index` and `archive_line`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutcomeRow {
    pub oracle_id: String,
    pub source_index: u32,
    pub archive_line: usize,
    pub status: BulkStatus,
    pub phase: OutcomePhase,
    pub reason_code: Option<String>,
    /// A JSON pointer into the archive record for read failures, or the
    /// compiler field path for lowering failures.
    pub location: Option<String>,
}

pub const OUTCOMES_MAX_BYTES: u64 = 32 * 1024 * 1024;
/// Each published file name with the proof of its staged bytes.
pub(crate) type StagedFiles = Vec<(&'static str, JsonByteProof)>;

/// A read, verified and assessed archive, ready to publish.
pub(crate) struct Intake {
    pub identity: &'static AcceptedArchive,
    pub catalog: Catalog,
    pub index: BulkIndex,
    pub counts: BTreeMap<BulkStatus, usize>,
    pub outcomes: Vec<OutcomeRow>,
}

/// Failure of the archive read (`Reader`) or of catalog validation (`Compiler`).
#[derive(Debug)]
pub(crate) enum IntakeError {
    Reader(String),
    Compiler(String),
}
impl IntakeError {
    pub fn into_message(self) -> String {
        match self {
            Self::Reader(e) | Self::Compiler(e) => e,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SourceBytes {
    source: Box<RawValue>,
    source_index: u32,
    source_sha256: String,
    ast_sha256: Option<String>,
    binding_sha256: Option<String>,
}

const BUFFER: usize = 64 * 1024;

fn read_line(reader: &mut impl BufRead, line: &mut Vec<u8>, max: usize) -> Result<usize, String> {
    line.clear();
    let read = reader
        .take(max as u64 + 1)
        .read_until(b'\n', line)
        .map_err(|e| e.to_string())?;
    if read > max {
        return Err("archive line byte limit exceeded".into());
    }
    Ok(read)
}

fn line_text(bytes: &[u8]) -> Result<&str, String> {
    // Match str::lines in coverage, while hashing the untouched input above.
    let trimmed = bytes
        .strip_suffix(b"\n")
        .map(|line| line.strip_suffix(b"\r").unwrap_or(line))
        .unwrap_or(bytes);
    std::str::from_utf8(trimmed).map_err(|e| e.to_string())
}

/// The assessed catalog, index, status counts and outcomes of one archive.
type IntakeRows = (
    Catalog,
    BulkIndex,
    BTreeMap<BulkStatus, usize>,
    Vec<OutcomeRow>,
);

fn intake(
    mut reader: impl BufRead,
    mut bytes: Vec<u8>,
    archive: &AcceptedArchive,
) -> Result<IntakeRows, String> {
    let file_pin = archive
        .file_sha256
        .as_deref()
        .ok_or("archive is missing a full-file pin")?;
    let mut hash = Sha256Sink::new();
    let mut evidence = crate::ingest::ArchiveEvidence::new(archive, bytes.len())?;
    hash.update(&bytes);
    let mut length = bytes.len() as u64;
    let mut definitions = Vec::new();
    let mut counts = BTreeMap::new();
    let mut rows = Vec::new();
    let mut outcomes = Vec::new();
    let mut ids = BTreeSet::new();
    loop {
        let read = read_line(&mut reader, &mut bytes, ARCHIVE_MAX_LINE_BYTES)?;
        if read == 0 {
            break;
        }
        let archive_line = rows.len() + 2;
        length += read as u64;
        if length > ARCHIVE_MAX_BYTES || rows.len() == ARCHIVE_MAX_RECORDS {
            return Err("archive size or record count limit exceeded".into());
        }
        hash.update(&bytes);
        let raw = line_text(&bytes)?;
        evidence.record(raw, read, archive_line)?;
        let row: Value =
            strict_read(raw).map_err(|e| format!("archive line {archive_line}: {e}"))?;
        let source = &row["source"];
        let id = row["oracleIdentity"]
            .as_str()
            .filter(|id| is_oracle_id(id))
            .ok_or_else(|| format!("invalid Oracle identity on archive line {archive_line}"))?;
        if !ids.insert(id.to_owned()) {
            return Err(format!(
                "duplicate Oracle identity on archive line {archive_line}: {id}"
            ));
        }
        if row["recordType"] != "card"
            || source["oracle_id"] != id
            || !matches!(
                row["status"].as_str(),
                Some("excluded" | "parsed" | "unsupported")
            )
        {
            return Err(format!(
                "invalid card envelope on archive line {archive_line}"
            ));
        }
        let source_bytes: SourceBytes = serde_json::from_str(raw).map_err(|e| e.to_string())?;
        if sha256(source_bytes.source.get().as_bytes()) != source_bytes.source_sha256 {
            return Err(format!("source hash drift on archive line {archive_line}"));
        }
        let facts = BulkCardFacts::from_source(source)?;
        let (status, reason_code, phase, location) = match assess_record(archive, raw, &row) {
            RecordAssessment::Definition(definition) => {
                definitions.push(definition);
                (
                    BulkStatus::LoweredValidated,
                    None,
                    OutcomePhase::Validate,
                    None,
                )
            }
            RecordAssessment::Gap {
                status,
                reason,
                phase,
                location,
            } => {
                if reason.is_empty()
                    || reason.len() > REASON_MAX_BYTES
                    || location
                        .as_ref()
                        .is_some_and(|l| l.is_empty() || l.len() > REASON_MAX_BYTES)
                {
                    return Err(format!(
                        "coverage reason size on archive line {archive_line}"
                    ));
                }
                (status, Some(reason), phase, location)
            }
        };
        outcomes.push(OutcomeRow {
            oracle_id: id.into(),
            source_index: source_bytes.source_index,
            archive_line,
            status,
            phase,
            reason_code: reason_code.clone(),
            location,
        });
        rows.push(BulkIndexRow {
            oracle_id: id.into(),
            facts,
            archive_line,
            raw_row_sha256: sha256(&bytes),
            record_sha256: sha256(raw.as_bytes()),
            source_index: source_bytes.source_index,
            source_sha256: source_bytes.source_sha256,
            ast_sha256: source_bytes.ast_sha256,
            binding_sha256: source_bytes.binding_sha256,
            status,
            reason_code,
        });
        *counts.entry(status).or_insert(0) += 1;
    }
    if hash.finalize_hex() != file_pin {
        return Err("archive full-file SHA-256 mismatch".into());
    }
    evidence.finish()?;
    definitions.sort_unstable_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    rows.sort_unstable_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    outcomes.sort_unstable_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    let catalog = Catalog {
        schema_version: CURRENT_IR_VERSION,
        rules_edition: RULES_EDITION.into(),
        rules_sha256: RULES_SHA256.into(),
        definitions,
    };
    Ok((catalog, BulkIndex { rows }, counts, outcomes))
}

/// Open, verify and assess an accepted archive. `Compiler` marks only the
/// final catalog validation; every other failure is the archive read.
pub(crate) fn read_archive(archive: &Path) -> Result<Intake, IntakeError> {
    let reader_error = IntakeError::Reader;
    let check = |metadata: fs::Metadata| {
        if !metadata.is_file() || metadata.len() > ARCHIVE_MAX_BYTES {
            return Err("invalid archive file or size".to_owned());
        }
        Ok(())
    };
    check(
        fs::symlink_metadata(archive)
            .map_err(|e| e.to_string())
            .map_err(reader_error)?,
    )
    .map_err(reader_error)?;
    let file = File::open(archive).map_err(|e| reader_error(e.to_string()))?;
    check(file.metadata().map_err(|e| reader_error(e.to_string()))?).map_err(reader_error)?;
    let mut reader = BufReader::with_capacity(BUFFER, file);
    let mut header = Vec::new();
    read_line(&mut reader, &mut header, ARCHIVE_MAX_LINE_BYTES).map_err(reader_error)?;
    let identity =
        accepted_manifest(line_text(&header).map_err(reader_error)?).map_err(reader_error)?;
    let (catalog, index, counts, outcomes) =
        intake(reader, header, identity).map_err(reader_error)?;
    catalog.validate().map_err(IntakeError::Compiler)?;
    Ok(Intake {
        identity,
        catalog,
        index,
        counts,
        outcomes,
    })
}

/// Write the three bulk format-2 files into `stage`.
pub(crate) fn write_bulk(
    stage: &Path,
    intake: &Intake,
    producer: &str,
) -> Result<BulkBuildReport, PayloadError> {
    let Intake {
        identity,
        catalog,
        index,
        counts,
        ..
    } = intake;
    let catalog_proof = write_payload(&stage.join("catalog.json"), CATALOG_MAX_BYTES, |w| {
        Ok(write_catalog_canonical(catalog, w)?)
    })?;
    let index_proof = write_payload(&stage.join("index.json"), INDEX_MAX_BYTES, |w| {
        Ok(serde_json::to_writer(w, index)?)
    })?;
    let manifest = BulkManifest {
        format_version: BULK_FORMAT_VERSION,
        ir_version: CURRENT_IR_VERSION,
        rules_edition: RULES_EDITION.into(),
        rules_sha256: RULES_SHA256.into(),
        producer_build_id: producer.into(),
        archive_sha256: identity
            .file_sha256
            .clone()
            .ok_or_else(|| "archive is missing a full-file pin".to_owned())?,
        archive_manifest_sha256: identity.manifest_sha256.clone(),
        parser_revision: identity.parser_revision.clone(),
        parser_source_sha256: identity.parser_source_sha256.clone(),
        catalog_digest: catalog_proof.sha256.clone(),
        catalog: catalog_proof,
        index: index_proof,
        outcome_counts: counts.clone(),
    };
    manifest.validate(producer)?;
    validate_index(&manifest, catalog, index)?;
    let manifest_bytes = canonical(&manifest)?;
    let proof = write_payload(&stage.join("manifest.json"), MANIFEST_MAX_BYTES, |w| {
        Ok(w.write_all(manifest_bytes.as_bytes())?)
    })?;
    Ok(BulkBuildReport {
        manifest_sha256: proof.sha256,
        manifest,
    })
}

/// Published file names and proofs of the three bulk format-2 files.
pub(crate) fn bulk_files(stage: &Path, report: &BulkBuildReport) -> Result<StagedFiles, String> {
    Ok(vec![
        ("catalog.json", report.manifest.catalog.clone()),
        ("index.json", report.manifest.index.clone()),
        (
            "manifest.json",
            JsonByteProof {
                bytes: fs::metadata(stage.join("manifest.json"))
                    .map_err(|e| e.to_string())?
                    .len(),
                sha256: report.manifest_sha256.clone(),
            },
        ),
    ])
}

pub fn build_ir(archive: &Path, out: &Path, producer: &str) -> Result<BulkBuildReport, String> {
    if !is_sha256(producer) {
        return Err("invalid producer identity".into());
    }
    let intake = read_archive(archive).map_err(IntakeError::into_message)?;
    publish(out, false, |stage| {
        let report = write_bulk(stage, &intake, producer).map_err(PayloadError::into_message)?;
        let files = bulk_files(stage, &report)?;
        Ok((report, files))
    })
    .map_err(PublishError::into_message)
}

pub(crate) struct ProofWriter {
    file: BufWriter<File>,
    limit: u64,
    bytes: u64,
    hash: Sha256Sink,
}
impl Write for ProofWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() as u64 > self.limit - self.bytes {
            return Err(io::Error::other("bulk output byte limit exceeded"));
        }
        let written = self.file.write(bytes)?;
        self.hash.update(&bytes[..written]);
        self.bytes += written as u64;
        Ok(written)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}
#[derive(Debug)]
pub(crate) enum PayloadError {
    Encode(String),
    Io(String),
}

impl PayloadError {
    fn into_message(self) -> String {
        match self {
            Self::Encode(e) | Self::Io(e) => e,
        }
    }
}

impl From<String> for PayloadError {
    fn from(error: String) -> Self {
        Self::Encode(error)
    }
}

impl From<io::Error> for PayloadError {
    fn from(error: io::Error) -> Self {
        Self::Io(error.to_string())
    }
}

impl From<serde_json::Error> for PayloadError {
    fn from(error: serde_json::Error) -> Self {
        if error.is_io() {
            Self::Io(error.to_string())
        } else {
            Self::Encode(error.to_string())
        }
    }
}

pub(crate) fn write_payload(
    path: &Path,
    limit: u64,
    encode: impl FnOnce(&mut ProofWriter) -> Result<(), PayloadError>,
) -> Result<JsonByteProof, PayloadError> {
    let file = OpenOptions::new().write(true).create_new(true).open(path)?;
    let mut writer = ProofWriter {
        file: BufWriter::with_capacity(BUFFER, file),
        limit,
        bytes: 0,
        hash: Sha256Sink::new(),
    };
    encode(&mut writer)?;
    writer.flush()?;
    writer.file.get_ref().sync_all()?;
    Ok(JsonByteProof {
        bytes: writer.bytes,
        sha256: writer.hash.finalize_hex(),
    })
}

fn matches_payload(path: &Path, proof: &JsonByteProof) -> bool {
    let read = || -> io::Result<bool> {
        let metadata = fs::symlink_metadata(path)?;
        if !metadata.is_file() || metadata.len() != proof.bytes {
            return Ok(false);
        }
        let mut hash = Sha256Sink::new();
        let size = io::copy(&mut File::open(path)?.take(proof.bytes + 1), &mut hash)?;
        Ok(size == proof.bytes && hash.finalize_hex() == proof.sha256)
    };
    read().unwrap_or(false)
}

/// A publication failure: the staged write itself, or exposing the bundle.
#[derive(Debug)]
pub(crate) enum PublishError<E> {
    Write(E, PathBuf),
    Destination(String),
}
impl PublishError<String> {
    fn into_message(self) -> String {
        match self {
            Self::Write(e, stage) => retained_stage(&e, &stage),
            Self::Destination(e) => e,
        }
    }
}

fn identical_files(left: &Path, right: &Path) -> io::Result<bool> {
    let meta = fs::symlink_metadata(right)?;
    if !meta.is_file() || meta.len() != fs::metadata(left)?.len() {
        return Ok(false);
    }
    let mut left = BufReader::with_capacity(BUFFER, File::open(left)?);
    let mut right = File::open(right)?;
    let mut buffer = [0; BUFFER];
    loop {
        let bytes = left.fill_buf()?;
        if bytes.is_empty() {
            return Ok(right.read(&mut buffer[..1])? == 0);
        }
        right.read_exact(&mut buffer[..bytes.len()])?;
        if bytes != &buffer[..bytes.len()] {
            return Ok(false);
        }
        let len = bytes.len();
        left.consume(len);
    }
}

fn contains_only(out: &Path, files: &StagedFiles) -> io::Result<bool> {
    let mut count = 0;
    for entry in fs::read_dir(out)? {
        let name = entry?.file_name();
        if !files.iter().any(|(expected, _)| name == *expected) {
            return Ok(false);
        }
        count += 1;
    }
    Ok(count == files.len())
}

fn sync_dir(path: &Path) -> Result<(), String> {
    File::open(path)
        .and_then(|dir| dir.sync_all())
        .map_err(|e| format!("cannot sync {}: {e}", path.display()))
}

const BUNDLE_FILES: [&str; 5] = [
    "catalog.json",
    "index.json",
    "manifest.json",
    "outcomes.jsonl",
    "compile-manifest.json",
];
const STAGE_PREFIX: &str = ".bulk-stage-";

fn stage_name(name: &std::ffi::OsStr) -> bool {
    let Some(suffix) = name
        .to_str()
        .and_then(|name| name.strip_prefix(STAGE_PREFIX))
    else {
        return false;
    };
    let mut parts = suffix.split('-');
    let (Some(digest), Some(pid), Some(nanos), Some(serial), None) = (
        parts.next(),
        parts.next(),
        parts.next(),
        parts.next(),
        parts.next(),
    ) else {
        return false;
    };
    digest.len() == 64
        && digest
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        && pid
            .parse::<u32>()
            .is_ok_and(|n| n != 0 && n.to_string() == pid)
        && u128::from_str_radix(nanos, 16).is_ok_and(|n| format!("{n:x}") == nanos)
        && u64::from_str_radix(serial, 16).is_ok_and(|n| format!("{n:x}") == serial)
}

/// List sibling stages without following directory symlinks or removing anything.
pub fn list_stages(parent: &Path) -> io::Result<Vec<PathBuf>> {
    let mut stages = Vec::new();
    for entry in fs::read_dir(parent)? {
        let entry = entry?;
        if stage_name(&entry.file_name()) && entry.file_type()?.is_dir() {
            stages.push(entry.path());
        }
    }
    stages.sort();
    Ok(stages)
}

/// Explicitly discard one abandoned stage. Inspect all entries before unlinking
/// anything, and never recurse into nested directories or follow leaf symlinks.
pub fn remove_stage(stage: &Path) -> io::Result<()> {
    let name = stage
        .file_name()
        .filter(|name| stage_name(name))
        .ok_or_else(|| io::Error::other("expected a generated .bulk-stage directory name"))?;
    let parent_path = stage
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC;
    let parent = openat(CWD, parent_path, flags, Mode::empty())?;
    let dir = openat(&parent, name, flags | OFlags::NOFOLLOW, Mode::empty())?;
    // Rebuild the path without trailing slashes or /. for no-follow checks.
    let stage = parent_path.join(name);
    remove_open_stage(&stage, &parent, name, &dir)
}

fn remove_open_stage(
    stage: &Path,
    parent: &rustix::fd::OwnedFd,
    name: &std::ffi::OsStr,
    dir: &rustix::fd::OwnedFd,
) -> io::Result<()> {
    let no_follow = AtFlags::SYMLINK_NOFOLLOW;
    let opened = fstat(dir)?;
    let check_identity = || -> io::Result<()> {
        for current in [
            statat(CWD, stage, no_follow)?,
            statat(parent, name, no_follow)?,
        ] {
            if (current.st_dev, current.st_ino) != (opened.st_dev, opened.st_ino) {
                return Err(io::Error::other("staging directory was replaced"));
            }
        }
        Ok(())
    };
    let regular = |entry: &std::ffi::CStr| -> io::Result<()> {
        if !BUNDLE_FILES
            .iter()
            .any(|name| entry.to_bytes() == name.as_bytes())
            || FileType::from_raw_mode(statat(dir, entry, no_follow)?.st_mode)
                != FileType::RegularFile
        {
            return Err(io::Error::other(
                "staging directory contains an unexpected entry",
            ));
        }
        Ok(())
    };
    let mut entries = Vec::new();
    for entry in Dir::read_from(dir)? {
        let entry = entry?;
        let name = entry.file_name();
        if name != c"." && name != c".." {
            regular(name)?;
            entries.push(name.to_owned());
        }
    }
    for entry in entries {
        regular(&entry)?;
        check_identity()?;
        unlinkat(dir, &entry, AtFlags::empty())?;
    }
    check_identity()?;
    // Files were unlinked through the verified handle. POSIX has no handle-bound
    // rmdir, so a concurrent swap here can remove only an empty replacement
    // directory and leaves any replacement files intact.
    unlinkat(parent, name, AtFlags::REMOVEDIR)?;
    Ok(())
}

fn create_stage(parent: &Path, name: &std::ffi::OsStr) -> io::Result<PathBuf> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let destination = sha256(name.as_encoded_bytes());
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_nanos();
    let pid = std::process::id();
    let serial = NEXT.fetch_add(1, Ordering::Relaxed);
    let stage = parent.join(format!(
        "{STAGE_PREFIX}{destination}-{pid}-{nanos:x}-{serial:x}"
    ));
    fs::DirBuilder::new().mode(0o700).create(&stage)?;
    Ok(stage)
}

pub(crate) fn retained_stage(error: &str, stage: &Path) -> String {
    format!("{error}; bulk stage left at {}", stage.display())
}

/// Expose one verified bundle atomically. A losing publisher compares the
/// occupied destination with its own staged bytes and leaves its stage for inspection.
pub(crate) fn publish<R, E>(
    out: &Path,
    exact: bool,
    write: impl FnOnce(&Path) -> Result<(R, StagedFiles), E>,
) -> Result<R, PublishError<E>> {
    use rustix::fs::{RenameFlags, renameat_with};

    let destination = |e: String| PublishError::Destination(e);
    let parent = out
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let name = out
        .file_name()
        .ok_or_else(|| destination("missing destination name".into()))?;
    if name.as_encoded_bytes().starts_with(STAGE_PREFIX.as_bytes()) {
        return Err(destination(
            "destination name is reserved for bundle staging".into(),
        ));
    }
    let out = parent.join(name);
    fs::create_dir_all(parent).map_err(|e| destination(format!("{}: {e}", parent.display())))?;
    let stage = create_stage(parent, name).map_err(|e| {
        destination(format!(
            "cannot create bulk stage in {}: {e}",
            parent.display()
        ))
    })?;
    let staged_destination = |e: String| destination(retained_stage(&e, &stage));
    #[cfg(test)]
    tests::publication_boundary("after-mkdir");
    let (report, files) = write(&stage).map_err(|e| PublishError::Write(e, stage.clone()))?;
    if let Some((name, _)) = files
        .iter()
        .find(|(name, proof)| !matches_payload(&stage.join(name), proof))
    {
        return Err(staged_destination(format!(
            "staged {name} does not match its proof"
        )));
    }
    if !contains_only(&stage, &files).unwrap_or(false) {
        return Err(staged_destination(
            "unexpected files in staged bundle".into(),
        ));
    }
    #[cfg(test)]
    tests::publication_boundary("after-verify");
    sync_dir(&stage).map_err(staged_destination)?;
    #[cfg(test)]
    tests::publication_boundary("before-rename");
    // NOREPLACE also protects an empty destination created concurrently.
    // https://docs.rs/rustix/1.1.6/rustix/fs/fn.renameat_with.html
    match renameat_with(CWD, &stage, CWD, &out, RenameFlags::NOREPLACE) {
        Ok(()) => {
            #[cfg(test)]
            tests::publication_boundary("after-rename");
            sync_dir(parent).map_err(destination)?;
            Ok(report)
        }
        Err(rustix::io::Errno::EXIST)
            if fs::symlink_metadata(&out).is_ok_and(|m| m.is_dir())
                && files.iter().all(|(name, _)| {
                    identical_files(&stage.join(name), &out.join(name)).unwrap_or(false)
                })
                && (!exact || contains_only(&out, &files).unwrap_or(false)) =>
        {
            sync_dir(parent).map_err(staged_destination)?;
            eprintln!(
                "identical bundle already published; bulk stage left at {}",
                stage.display()
            );
            Ok(report)
        }
        Err(error) => Err(staged_destination(format!(
            "cannot publish bulk artifact to {}: {error}; an existing destination must hold the identical bundle",
            out.display()
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn stage1d_bulk_intake_rejects_oversize_duplicate_identity_and_hash_drift() {
        let mut reader = Cursor::new(b"0123456789\n");
        assert!(
            read_line(&mut reader, &mut Vec::new(), 5)
                .unwrap_err()
                .contains("limit")
        );
        let header = include_str!("../../../tests/fixtures/archive-manifest-phase1-r5.json")
            .as_bytes()
            .to_vec();
        let mut accepted = accepted_manifest(line_text(&header).unwrap())
            .unwrap()
            .clone();
        let raw =
            include_str!("../../../tests/fixtures/phase1-r5/compose-strip-mine.json").as_bytes();
        accepted.file_sha256 = Some(sha256(&[header.as_slice(), raw].concat()));
        let (catalog, index, _, outcomes) =
            intake(Cursor::new(raw), header.clone(), &accepted).unwrap();
        assert_eq!(outcomes.len(), 1);
        assert_eq!(
            (outcomes[0].archive_line, outcomes[0].phase),
            (index.rows[0].archive_line, OutcomePhase::Validate)
        );
        assert_eq!(catalog.definitions.len(), 1);
        assert_eq!(index.rows[0].raw_row_sha256, sha256(raw));
        assert!(
            intake(Cursor::new([raw, raw].concat()), header.clone(), &accepted)
                .unwrap_err()
                .contains("duplicate Oracle identity")
        );
        let drift = [raw.strip_suffix(b"\n").unwrap(), b" \n"].concat();
        assert!(
            intake(Cursor::new(drift), header, &accepted)
                .unwrap_err()
                .contains("full-file SHA-256 mismatch")
        );
    }

    /// Stage `names`, each holding `body` followed by its own name.
    fn stage_files(
        stage: &Path,
        names: &[&'static str],
        body: &str,
    ) -> Result<((), StagedFiles), String> {
        let mut files = vec![];
        for name in names {
            let proof = write_payload(&stage.join(name), 64, |w| {
                Ok(w.write_all(format!("{body}{name}").as_bytes())?)
            })
            .map_err(PayloadError::into_message)?;
            files.push((*name, proof));
        }
        Ok(((), files))
    }

    fn scratch(label: &str) -> std::path::PathBuf {
        for attempt in 0.. {
            let parent =
                std::env::temp_dir().join(format!("mtg-{label}-{}-{attempt}", std::process::id()));
            match fs::create_dir(&parent) {
                Ok(()) => return parent,
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(e) => panic!("cannot create {}: {e}", parent.display()),
            }
        }
        unreachable!()
    }

    #[test]
    fn stage1d_bulk_publication_keeps_occupied_receipts_and_failed_stage() {
        let parent = scratch("bulk-publish");
        let dest = parent.join("bulk");
        let write =
            |stage: &Path| stage_files(stage, &["catalog.json", "index.json", "manifest.json"], "");
        publish(&dest, false, write).unwrap();
        publish(&dest, false, write).unwrap();
        fs::write(dest.join("index.json"), b"preserve").unwrap();
        let PublishError::Destination(error) = publish(&dest, false, write).unwrap_err() else {
            panic!("expected a destination failure");
        };
        assert!(error.contains("cannot publish"));
        let stages = list_stages(&parent).unwrap();
        assert_eq!(stages.len(), 2);
        assert!(
            stages
                .iter()
                .any(|stage| error.ends_with(stage.to_str().unwrap()))
        );
        assert_eq!(fs::read(dest.join("index.json")).unwrap(), b"preserve");
        fs::remove_dir_all(parent).unwrap();
    }

    const BUNDLE: [&str; 5] = BUNDLE_FILES;

    #[test]
    fn bundle_retry_requires_all_five_files_and_nothing_else() {
        let parent = scratch("bundle-retry");
        let dest = parent.join("bundle");
        let write = |stage: &Path| stage_files(stage, &BUNDLE, "");
        publish(&dest, true, write).unwrap();
        // An identical retry succeeds and leaves its unpublished stage.
        publish(&dest, true, write).unwrap();
        let stages = list_stages(&parent).unwrap();
        assert_eq!(stages.len(), 1);
        assert_bundle(&stages[0], "");
        // The same three bulk files with a corrupt outcomes file reject.
        fs::write(dest.join("outcomes.jsonl"), b"corrupt").unwrap();
        assert!(matches!(
            publish(&dest, true, write),
            Err(PublishError::Destination(_))
        ));
        assert_eq!(fs::read(dest.join("outcomes.jsonl")).unwrap(), b"corrupt");
        fs::write(dest.join("outcomes.jsonl"), b"outcomes.jsonl").unwrap();
        publish(&dest, true, write).unwrap();
        // An extra entry is a conflicting destination for an exact bundle.
        fs::write(dest.join("notes"), b"keep").unwrap();
        assert!(matches!(
            publish(&dest, true, write),
            Err(PublishError::Destination(_))
        ));
        assert_eq!(fs::read(dest.join("notes")).unwrap(), b"keep");
        // A different bundle never replaces an occupied destination.
        fs::remove_file(dest.join("notes")).unwrap();
        let other = |stage: &Path| stage_files(stage, &BUNDLE, "other ");
        assert!(publish(&dest, true, other).is_err());
        assert_eq!(
            fs::read(dest.join("catalog.json")).unwrap(),
            b"catalog.json"
        );
        fs::remove_dir_all(parent).unwrap();
    }

    #[test]
    fn failed_or_tampered_stage_exposes_nothing() {
        let parent = scratch("bundle-failed");
        let dest = parent.join("bundle");
        let failed = publish(&dest, true, |stage: &Path| {
            stage_files(stage, &BUNDLE[..3], "")?;
            Err::<((), StagedFiles), _>("interrupted".to_owned())
        });
        let Err(PublishError::Write(e, stage)) = failed else {
            panic!("expected a staged write failure");
        };
        assert_eq!(e, "interrupted");
        assert_eq!(fs::read_dir(stage).unwrap().count(), 3);
        let tampered = publish(&dest, true, |stage: &Path| {
            let staged = stage_files(stage, &BUNDLE, "")?;
            fs::write(stage.join("outcomes.jsonl"), b"tampered").map_err(|e| e.to_string())?;
            Ok::<_, String>(staged)
        });
        assert!(matches!(tampered, Err(PublishError::Destination(_))));
        assert!(!dest.exists());
        assert_eq!(list_stages(&parent).unwrap().len(), 2);
        fs::remove_dir_all(parent).unwrap();
    }

    #[test]
    fn failed_write_preserves_stage_replacement() {
        let parent = scratch("bundle-stage-swap");
        let dest = parent.join("bundle");
        let moved = parent.join("moved");
        let mut original_path = PathBuf::new();
        let result = publish(&dest, true, |stage| {
            original_path = stage.to_owned();
            fs::rename(stage, &moved).unwrap();
            fs::create_dir(stage).unwrap();
            fs::write(stage.join("catalog.json"), b"foreign").unwrap();
            stage_files(stage, &BUNDLE, "")
        });
        let error = result.unwrap_err().into_message();
        assert!(error.ends_with(original_path.to_str().unwrap()));
        assert_eq!(
            fs::read(original_path.join("catalog.json")).unwrap(),
            b"foreign"
        );
        assert!(moved.is_dir());
        assert!(!dest.exists());
        fs::remove_dir_all(parent).unwrap();
    }

    #[test]
    fn concurrent_publishers_keep_the_winner_and_leave_foreign_stages_alone() {
        use std::os::unix::fs::MetadataExt;
        let parent = scratch("bundle-race");
        let dest = parent.join("bundle");
        let foreign = create_stage(&parent, dest.file_name().unwrap()).unwrap();
        stage_files(&foreign, &BUNDLE, "foreign ").unwrap();
        let rendezvous = std::sync::Barrier::new(2);
        std::thread::scope(|scope| {
            let publish_bundle = || {
                publish(&dest, true, |stage| {
                    let files = stage_files(stage, &BUNDLE, "")?;
                    let inode = fs::metadata(stage).unwrap().ino();
                    rendezvous.wait();
                    Ok::<_, String>((inode, files.1))
                })
                .unwrap()
            };
            let first = scope.spawn(publish_bundle);
            let second = scope.spawn(publish_bundle);
            let inodes = [first.join().unwrap(), second.join().unwrap()];
            assert_ne!(inodes[0], inodes[1]);
            assert!(inodes.contains(&fs::metadata(&dest).unwrap().ino()));
        });
        assert_bundle(&dest, "");
        assert_bundle(&foreign, "foreign ");
        let stages = list_stages(&parent).unwrap();
        assert_eq!(stages.len(), 2);
        assert!(stages.contains(&foreign));
        let retry = stages.iter().find(|stage| **stage != foreign).unwrap();
        assert_bundle(retry, "");
        fs::remove_dir_all(parent).unwrap();
    }

    #[test]
    fn bundle_never_replaces_an_empty_destination() {
        use std::os::unix::fs::MetadataExt;
        let parent = scratch("bundle-occupied");
        let dest = parent.join("bundle");
        let mut occupied_inode = 0;
        let result = publish(&dest, true, |stage| {
            let result = stage_files(stage, &BUNDLE, "")?;
            fs::create_dir(&dest).unwrap();
            occupied_inode = fs::metadata(&dest).unwrap().ino();
            Ok::<_, String>(result)
        });
        assert!(matches!(result, Err(PublishError::Destination(_))));
        assert_eq!(fs::metadata(&dest).unwrap().ino(), occupied_inode);
        assert_eq!(fs::read_dir(&dest).unwrap().count(), 0);
        assert_eq!(list_stages(&parent).unwrap().len(), 1);
        fs::remove_dir_all(parent).unwrap();
    }

    #[test]
    fn explicit_stage_removal_refuses_unknown_entries_and_symlinks() {
        let parent = scratch("remove-stage");
        let stage = create_stage(&parent, "bundle".as_ref()).unwrap();
        let foreign = parent.join("foreign");
        fs::create_dir(&foreign).unwrap();
        let regular = stage.join("index.json");
        let unexpected = stage.join("unexpected");
        let symlink = stage.join("catalog.json");
        let linked_dir = create_stage(&parent, "bundle".as_ref()).unwrap();
        fs::remove_dir(&linked_dir).unwrap();
        let backup = parent.join(".bulk-stage-customer-backup");
        fs::create_dir(&backup).unwrap();
        fs::write(backup.join("catalog.json"), b"backup").unwrap();
        fs::write(&regular, b"staged").unwrap();
        fs::write(&unexpected, b"unknown").unwrap();
        fs::write(foreign.join("catalog.json"), b"foreign").unwrap();
        std::os::unix::fs::symlink(foreign.join("catalog.json"), &symlink).unwrap();
        std::os::unix::fs::symlink(&foreign, &linked_dir).unwrap();

        assert_eq!(
            list_stages(&parent).unwrap().as_slice(),
            std::slice::from_ref(&stage)
        );
        assert!(remove_stage(&stage).is_err());
        assert_eq!(fs::read(&regular).unwrap(), b"staged");
        fs::remove_file(&unexpected).unwrap();
        assert!(remove_stage(&stage).is_err());
        assert!(remove_stage(&linked_dir.join(".")).is_err());
        assert!(remove_stage(&foreign).is_err());
        assert!(remove_stage(&backup).is_err());
        assert_eq!(fs::read(backup.join("catalog.json")).unwrap(), b"backup");
        assert_eq!(fs::read(foreign.join("catalog.json")).unwrap(), b"foreign");
        fs::remove_file(symlink).unwrap();
        let nested = stage.join("catalog.json");
        fs::create_dir(&nested).unwrap();
        assert!(remove_stage(&stage).is_err());
        assert_eq!(fs::read(&regular).unwrap(), b"staged");
        fs::remove_dir(nested).unwrap();
        remove_stage(&stage).unwrap();
        assert!(!stage.exists());
        fs::remove_dir_all(parent).unwrap();
    }

    #[test]
    fn stage_names_match_only_the_generated_form() {
        let digest = sha256(b"bundle");
        assert!(stage_name(
            format!("{STAGE_PREFIX}{digest}-42-abc-0").as_ref()
        ));
        for suffix in [
            format!("{digest}-0-abc-0"),
            format!("{digest}-042-abc-0"),
            format!("{digest}-42-ABC-0"),
            format!("{digest}-42-0abc-0"),
            format!("{digest}-42-abc-00"),
            format!("{digest}-42-abc-10000000000000000"),
            format!("{digest}-42-abc-0-extra"),
            format!("{}-42-abc-0", digest.to_uppercase()),
        ] {
            assert!(!stage_name(format!("{STAGE_PREFIX}{suffix}").as_ref()));
        }
    }

    #[test]
    fn explicit_stage_removal_preserves_replacement() {
        let parent_path = scratch("remove-stage-swap");
        let stage = create_stage(&parent_path, "bundle".as_ref()).unwrap();
        let moved = parent_path.join("moved");
        let name = stage.file_name().unwrap();
        let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC;
        let parent = openat(CWD, &parent_path, flags, Mode::empty()).unwrap();
        let dir = openat(&parent, name, flags, Mode::empty()).unwrap();
        fs::write(stage.join("catalog.json"), b"original").unwrap();
        fs::rename(&stage, &moved).unwrap();
        fs::create_dir(&stage).unwrap();
        fs::write(stage.join("catalog.json"), b"replacement").unwrap();
        let error = remove_open_stage(&stage, &parent, name, &dir).unwrap_err();
        assert!(error.to_string().contains("was replaced"));
        assert_eq!(fs::read(moved.join("catalog.json")).unwrap(), b"original");
        assert_eq!(
            fs::read(stage.join("catalog.json")).unwrap(),
            b"replacement"
        );
        fs::remove_dir_all(parent_path).unwrap();
    }

    fn assert_bundle(path: &Path, body: &str) {
        assert_eq!(fs::read_dir(path).unwrap().count(), BUNDLE.len());
        for name in BUNDLE {
            assert_eq!(
                fs::read(path.join(name)).unwrap(),
                format!("{body}{name}").as_bytes()
            );
        }
    }

    const PAUSE_AT: &str = "MTG_TEST_BULK_PAUSE_AT";
    const CHILD_DEST: &str = "MTG_TEST_BULK_DEST";

    pub(super) fn publication_boundary(site: &str) {
        if std::env::var(PAUSE_AT).as_deref() == Ok(site) {
            println!("ready for SIGKILL at {site}");
            loop {
                std::thread::park();
            }
        }
    }

    #[test]
    fn publication_kill_child() {
        if let Some(out) = std::env::var_os(CHILD_DEST) {
            publish(Path::new(&out), true, |stage| {
                if std::env::var(PAUSE_AT).as_deref() == Ok("mid-write") {
                    write_payload(&stage.join("catalog.json"), 64, |writer| {
                        writer.write_all(b"partial")?;
                        writer.flush()?;
                        publication_boundary("mid-write");
                        Ok(())
                    })
                    .map_err(PayloadError::into_message)?;
                    panic!("mid-write hook returned");
                }
                stage_files(stage, &BUNDLE, "")
            })
            .unwrap();
        }
    }

    #[test]
    fn sigkill_at_each_publication_boundary_leaves_retryable_output() {
        use std::{
            os::unix::process::ExitStatusExt,
            process::{Command, Stdio},
            sync::mpsc,
            time::Duration,
        };

        for site in [
            "after-mkdir",
            "mid-write",
            "after-verify",
            "before-rename",
            "after-rename",
        ] {
            let parent = scratch(site);
            let dest = parent.join("bundle");
            let mut child = Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "bulk::tests::publication_kill_child",
                    "--nocapture",
                ])
                .env(PAUSE_AT, site)
                .env(CHILD_DEST, &dest)
                .stdout(Stdio::piped())
                .spawn()
                .unwrap();
            let stdout = child.stdout.take().unwrap();
            let (send, receive) = mpsc::channel();
            let reader = std::thread::spawn(move || {
                let message = format!("ready for SIGKILL at {site}");
                let reached = BufReader::new(stdout)
                    .lines()
                    .map_while(Result::ok)
                    .any(|line| line.contains(&message));
                send.send(reached).ok();
            });
            let reached = receive.recv_timeout(Duration::from_secs(15));
            child.kill().unwrap();
            let status = child.wait().unwrap();
            reader.join().unwrap();
            assert_eq!(reached, Ok(true), "{site}");
            assert_eq!(status.signal(), Some(9), "{site}");
            assert_eq!(dest.exists(), site == "after-rename", "{site}");
            if dest.exists() {
                assert_bundle(&dest, "");
            }
            let stages = list_stages(&parent).unwrap();
            assert_eq!(stages.len(), usize::from(site != "after-rename"), "{site}");
            publish(&dest, true, |stage| stage_files(stage, &BUNDLE, "")).unwrap();
            assert_bundle(&dest, "");
            let retained = list_stages(&parent).unwrap();
            assert_eq!(retained.len(), 1, "{site}");
            assert!(
                stages.iter().all(|stage| retained.contains(stage)),
                "{site}"
            );
            for stage in retained {
                remove_stage(&stage).unwrap();
            }
            fs::remove_dir_all(parent).unwrap();
        }
    }
}
