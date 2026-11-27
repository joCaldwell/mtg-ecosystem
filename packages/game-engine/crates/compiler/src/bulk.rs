//! Compile an accepted archive into one pinned, compact bulk artifact.
use crate::coverage::{RecordAssessment, assess_record};
use mtg_contracts::{bulk_catalog::*, *};
use serde::{Deserialize, Serialize};
use serde_json::{Value, value::RawValue};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::{self, BufRead, BufReader, BufWriter, Read, Write},
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
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
    Write(E),
    Destination(String),
}
impl PublishError<String> {
    fn into_message(self) -> String {
        match self {
            Self::Write(e) | Self::Destination(e) => e,
        }
    }
}

/// Whether `out` already holds exactly these published bytes. With `exact`,
/// the directory must contain no other entry.
fn matches_bundle(out: &Path, files: &StagedFiles, exact: bool) -> bool {
    fs::symlink_metadata(out).is_ok_and(|m| m.is_dir())
        && files
            .iter()
            .all(|(name, proof)| matches_payload(&out.join(name), proof))
        && (!exact
            || fs::read_dir(out).is_ok_and(|entries| {
                let names = entries
                    .map(|e| e.map(|e| e.file_name()))
                    .collect::<Result<BTreeSet<_>, _>>();
                names.is_ok_and(|names| {
                    names.len() == files.len()
                        && files
                            .iter()
                            .all(|(name, _)| names.contains(std::ffi::OsStr::new(name)))
                })
            }))
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
const STAGE_OWNER: &str = "owner";
const STAGE_BUNDLE: &str = "bundle";

fn stage_owner(parent: &fs::Metadata, stage: &fs::Metadata, destination: &str) -> String {
    // Bind the marker to the locked parent, destination name and this stage's
    // filesystem identity. Copying the marker into another directory is insufficient.
    format!(
        "bulk-stage-v1\n{}:{}\n{destination}\n{}:{}\n",
        parent.dev(),
        parent.ino(),
        stage.dev(),
        stage.ino(),
    )
}

fn create_stage(stage: &Path, parent: &fs::Metadata, destination: &str) -> io::Result<PathBuf> {
    fs::DirBuilder::new().mode(0o700).create(stage)?;
    let owner = stage_owner(parent, &fs::symlink_metadata(stage)?, destination);
    let mut marker = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(stage.join(STAGE_OWNER))?;
    marker.write_all(owner.as_bytes())?;
    marker.sync_all()?;
    let bundle = stage.join(STAGE_BUNDLE);
    fs::DirBuilder::new().mode(0o700).create(&bundle)?;
    File::open(stage)?.sync_all()?;
    Ok(bundle)
}

fn remove_stage(stage: &Path, parent: &fs::Metadata, destination: &str) -> io::Result<()> {
    let metadata = match fs::symlink_metadata(stage) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        result => result?,
    };
    if !metadata.is_dir() || metadata.permissions().mode() & 0o777 != 0o700 {
        return Err(io::Error::other("staging path is not a private directory"));
    }
    let owner = stage_owner(parent, &metadata, destination);
    let marker = stage.join(STAGE_OWNER);
    if !matches_payload(
        &marker,
        &JsonByteProof {
            bytes: owner.len() as u64,
            sha256: sha256(owner.as_bytes()),
        },
    ) {
        return Err(io::Error::other(
            "staging directory has no valid ownership marker",
        ));
    }
    let mut bundle = None;
    for entry in fs::read_dir(stage)? {
        let entry = entry?;
        let name = entry.file_name();
        if name == STAGE_OWNER {
            continue;
        }
        if name != STAGE_BUNDLE || !entry.file_type()?.is_dir() {
            return Err(io::Error::other(
                "staging directory contains an unexpected entry",
            ));
        }
        bundle = Some(entry.path());
    }
    if let Some(bundle) = bundle {
        let entries = fs::read_dir(&bundle)?.collect::<Result<Vec<_>, _>>()?;
        for entry in &entries {
            if !entry.file_type()?.is_file()
                || !BUNDLE_FILES.iter().any(|name| entry.file_name() == *name)
            {
                return Err(io::Error::other(
                    "staged bundle contains an unexpected entry",
                ));
            }
        }
        for entry in entries {
            fs::remove_file(entry.path())?;
        }
        fs::remove_dir(bundle)?;
    }
    fs::remove_file(marker)?;
    fs::remove_dir(stage)
}

/// Write files under a private 0700 sibling stage, re-verify each against its
/// proof, sync the bundle, rename it onto `out` once and sync the parent.
/// When the rename fails, success requires `out` to already hold the same
/// files (exactly those files when `exact`): an identical retry or a
/// concurrent identical writer. Under the parent lock, a retry only discards
/// stages with ownership evidence. The marker stays outside the renamed bundle.
pub(crate) fn publish<R, E>(
    out: &Path,
    exact: bool,
    write: impl FnOnce(&Path) -> Result<(R, StagedFiles), E>,
) -> Result<R, PublishError<E>> {
    let destination = |e: String| PublishError::Destination(e);
    let parent = out
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let name = out
        .file_name()
        .ok_or_else(|| destination("missing destination name".into()))?;
    if name.as_encoded_bytes().starts_with(b".bulk-stage-") {
        return Err(destination(
            "destination name is reserved for bundle staging".into(),
        ));
    }
    let io_error = |e: io::Error| destination(e.to_string());
    fs::create_dir_all(parent).map_err(io_error)?;
    let lock = File::open(parent).map_err(io_error)?;
    lock.lock().map_err(io_error)?;
    let parent_metadata = lock.metadata().map_err(io_error)?;
    let destination_id = sha256(name.as_encoded_bytes());
    let stage = parent.join(format!(".bulk-stage-{destination_id}"));
    let cleanup = || {
        remove_stage(&stage, &parent_metadata, &destination_id)
            .map_err(|e| destination(format!("stale or foreign stage {}: {e}", stage.display())))
    };
    cleanup()?;
    let bundle = create_stage(&stage, &parent_metadata, &destination_id)
        .map_err(|e| destination(format!("cannot create bulk stage {}: {e}", stage.display())))?;
    let result = (|| {
        let (report, files) = write(&bundle).map_err(PublishError::Write)?;
        if let Some((name, _)) = files
            .iter()
            .find(|(name, proof)| !matches_payload(&bundle.join(name), proof))
        {
            return Err(destination(format!(
                "staged {name} does not match its proof"
            )));
        }
        sync_dir(&bundle).map_err(destination)?;
        let published = match fs::symlink_metadata(out) {
            Ok(_) => Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "destination already exists",
            )),
            Err(e) if e.kind() == io::ErrorKind::NotFound => fs::rename(&bundle, out),
            Err(e) => Err(e),
        };
        if let Err(error) = published
            && !matches_bundle(out, &files, exact)
        {
            return Err(destination(format!(
                "cannot publish bulk artifact to {}: {error}",
                out.display()
            )));
        }
        Ok(report)
    })();
    cleanup()?;
    let report = result?;
    sync_dir(parent).map_err(destination)?;
    Ok(report)
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
    fn stage1d_bulk_publication_keeps_occupied_receipts_and_cleans_failed_stage() {
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
        assert_eq!(fs::read(dest.join("index.json")).unwrap(), b"preserve");
        assert_eq!(fs::read_dir(&parent).unwrap().count(), 1);
        fs::remove_dir_all(parent).unwrap();
    }

    const BUNDLE: [&str; 5] = BUNDLE_FILES;

    #[test]
    fn bundle_retry_requires_all_five_files_and_nothing_else() {
        let parent = scratch("bundle-retry");
        let dest = parent.join("bundle");
        let write = |stage: &Path| stage_files(stage, &BUNDLE, "");
        publish(&dest, true, write).unwrap();
        // An identical retry succeeds and leaves no staging debris.
        publish(&dest, true, write).unwrap();
        assert_eq!(fs::read_dir(&parent).unwrap().count(), 1);
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
    fn bundle_interrupted_write_exposes_nothing_and_concurrent_writers_agree() {
        let parent = scratch("bundle-race");
        let dest = parent.join("bundle");
        // A failure after some files were staged publishes nothing.
        let failed = publish(&dest, true, |stage: &Path| {
            stage_files(stage, &BUNDLE[..3], "")?;
            Err::<((), StagedFiles), _>("interrupted".to_owned())
        });
        assert!(matches!(failed, Err(PublishError::Write(e)) if e == "interrupted"));
        assert!(!dest.exists());
        assert_eq!(fs::read_dir(&parent).unwrap().count(), 0);
        // A staged file that no longer matches its proof is never exposed.
        let tampered = publish(&dest, true, |stage: &Path| {
            let staged = stage_files(stage, &BUNDLE, "")?;
            fs::write(stage.join("outcomes.jsonl"), b"tampered").map_err(|e| e.to_string())?;
            Ok::<_, String>(staged)
        });
        assert!(matches!(tampered, Err(PublishError::Destination(_))));
        assert!(!dest.exists());
        // Leave a real stage in the state a writer killed during its write would leave.
        let id = sha256(b"bundle");
        let abandoned = parent.join(format!(".bulk-stage-{id}"));
        let bundle = create_stage(&abandoned, &fs::metadata(&parent).unwrap(), &id).unwrap();
        fs::write(bundle.join("catalog.json"), b"incomplete").unwrap();
        // Identical concurrent writers all succeed with one complete bundle.
        std::thread::scope(|scope| {
            let writers: Vec<_> = (0..4)
                .map(|_| {
                    scope.spawn(|| {
                        publish(&dest, true, |stage: &Path| stage_files(stage, &BUNDLE, ""))
                    })
                })
                .collect();
            for writer in writers {
                writer.join().unwrap().unwrap();
            }
        });
        assert_eq!(fs::read_dir(&dest).unwrap().count(), BUNDLE.len());
        assert_eq!(fs::read_dir(&parent).unwrap().count(), 1);
        fs::remove_dir_all(parent).unwrap();
    }

    #[test]
    fn bundle_retry_preserves_a_foreign_bundle_at_the_stage_path() {
        let parent = scratch("foreign-stage");
        let dest = parent.join("bundle");
        let id = sha256(b"bundle");
        let stage = parent.join(format!(".bulk-stage-{id}"));
        let write = |stage: &Path| stage_files(stage, &BUNDLE, "");
        fs::DirBuilder::new().mode(0o700).create(&stage).unwrap();
        let (_, files) = write(&stage).unwrap();
        let other =
            create_stage(&parent.join("other"), &fs::metadata(&parent).unwrap(), &id).unwrap();
        for add_marker in [false, true] {
            if add_marker {
                // Even a well-formed marker for this destination binds the stage that made it.
                fs::copy(other.with_file_name(STAGE_OWNER), stage.join(STAGE_OWNER)).unwrap();
            }
            let PublishError::Destination(error) = publish(&dest, true, write).unwrap_err() else {
                panic!("expected a destination failure");
            };
            assert!(error.contains(&format!("stale or foreign stage {}", stage.display())));
            assert!(!dest.exists());
            assert!(
                files
                    .iter()
                    .all(|(name, proof)| matches_payload(&stage.join(name), proof))
            );
        }
        fs::remove_dir_all(parent).unwrap();
    }

    #[test]
    fn bundle_retry_preserves_unknown_staging_entries_and_empty_destinations() {
        let parent = scratch("bundle-occupied");
        let dest = parent.join("bundle");
        let write = |stage: &Path| stage_files(stage, &BUNDLE, "");
        let id = sha256(b"bundle");
        let stage = parent.join(format!(".bulk-stage-{id}"));
        let bundle = create_stage(&stage, &fs::metadata(&parent).unwrap(), &id).unwrap();
        let (_, staged) = write(&bundle).unwrap();
        let unrelated = bundle.join("keep");
        fs::write(&unrelated, b"keep").unwrap();
        assert!(matches!(
            publish(&dest, true, write),
            Err(PublishError::Destination(_))
        ));
        assert_eq!(fs::read(&unrelated).unwrap(), b"keep");
        assert!(matches_payload(&bundle.join(staged[0].0), &staged[0].1));
        fs::remove_dir_all(stage).unwrap();
        fs::create_dir(&dest).unwrap();
        assert!(matches!(
            publish(&dest, true, write),
            Err(PublishError::Destination(_))
        ));
        assert_eq!(fs::read_dir(&dest).unwrap().count(), 0);
        assert_eq!(fs::read_dir(&parent).unwrap().count(), 1);
        fs::remove_dir_all(parent).unwrap();
    }
}
