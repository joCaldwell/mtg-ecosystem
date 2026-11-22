//! Compile an accepted archive into one pinned, compact bulk artifact.
use crate::coverage::{RecordAssessment, assess_record};
use mtg_contracts::{bulk_catalog::*, *};
use serde::{Deserialize, Serialize};
use serde_json::{Value, value::RawValue};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::{self, BufRead, BufReader, BufWriter, Read, Write},
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

#[derive(Debug, Serialize)]
pub struct BulkBuildReport {
    pub manifest_sha256: String,
    pub manifest: BulkManifest,
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

fn intake(
    mut reader: impl BufRead,
    mut bytes: Vec<u8>,
    archive: &AcceptedArchive,
) -> Result<(Catalog, BulkIndex, BTreeMap<BulkStatus, usize>), String> {
    let file_pin = archive
        .file_sha256
        .as_deref()
        .ok_or("archive is missing a full-file pin")?;
    let mut hash = Sha256Sink::new();
    hash.update(&bytes);
    let mut length = bytes.len() as u64;
    let mut definitions = Vec::new();
    let mut counts = BTreeMap::new();
    let mut rows = Vec::new();
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
        let (status, reason_code) = match assess_record(archive, raw, &row) {
            RecordAssessment::Definition(definition) => {
                definitions.push(definition);
                (BulkStatus::LoweredValidated, None)
            }
            RecordAssessment::Gap { status, reason } => {
                if reason.is_empty() || reason.len() > REASON_MAX_BYTES {
                    return Err(format!(
                        "coverage reason size on archive line {archive_line}"
                    ));
                }
                (status, Some(reason))
            }
        };
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
    definitions.sort_unstable_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    rows.sort_unstable_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    let catalog = Catalog {
        schema_version: CURRENT_IR_VERSION,
        rules_edition: RULES_EDITION.into(),
        rules_sha256: RULES_SHA256.into(),
        definitions,
    };
    catalog.validate()?;
    Ok((catalog, BulkIndex { rows }, counts))
}

pub fn build_ir(archive: &Path, out: &Path, producer: &str) -> Result<BulkBuildReport, String> {
    let check = |metadata: fs::Metadata| {
        if !metadata.is_file() || metadata.len() > ARCHIVE_MAX_BYTES {
            return Err("invalid archive file or size".to_owned());
        }
        Ok(())
    };
    if !is_sha256(producer) {
        return Err("invalid producer identity".into());
    }
    check(fs::symlink_metadata(archive).map_err(|e| e.to_string())?)?;
    let file = File::open(archive).map_err(|e| e.to_string())?;
    check(file.metadata().map_err(|e| e.to_string())?)?;
    let mut reader = BufReader::with_capacity(BUFFER, file);
    let mut header = Vec::new();
    read_line(&mut reader, &mut header, ARCHIVE_MAX_LINE_BYTES)?;
    let identity = accepted_manifest(line_text(&header)?)?;
    let (catalog, index, counts) = intake(reader, header, identity)?;
    publish(out, |stage| {
        let catalog_proof = write_payload(&stage.join("catalog.json"), CATALOG_MAX_BYTES, |w| {
            write_catalog_canonical(&catalog, w)
        })?;
        let index_proof = write_payload(&stage.join("index.json"), INDEX_MAX_BYTES, |w| {
            serde_json::to_writer(w, &index).map_err(|e| e.to_string())
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
                .ok_or("archive is missing a full-file pin")?,
            archive_manifest_sha256: identity.manifest_sha256.clone(),
            parser_revision: identity.parser_revision.clone(),
            parser_source_sha256: identity.parser_source_sha256.clone(),
            catalog_digest: catalog_proof.sha256.clone(),
            catalog: catalog_proof,
            index: index_proof,
            outcome_counts: counts,
        };
        manifest.validate(producer)?;
        validate_index(&manifest, &catalog, &index)?;
        let proof = write_payload(&stage.join("manifest.json"), MANIFEST_MAX_BYTES, |w| {
            w.write_all(canonical(&manifest)?.as_bytes())
                .map_err(|e| e.to_string())
        })?;
        Ok(BulkBuildReport {
            manifest_sha256: proof.sha256,
            manifest,
        })
    })
}

struct ProofWriter {
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
fn write_payload(
    path: &Path,
    limit: u64,
    encode: impl FnOnce(&mut ProofWriter) -> Result<(), String>,
) -> Result<JsonByteProof, String> {
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    let mut writer = ProofWriter {
        file: BufWriter::with_capacity(BUFFER, file),
        limit,
        bytes: 0,
        hash: Sha256Sink::new(),
    };
    encode(&mut writer)?;
    writer.flush().map_err(|e| e.to_string())?;
    writer
        .file
        .get_ref()
        .sync_all()
        .map_err(|e| e.to_string())?;
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

fn publish(
    out: &Path,
    write: impl FnOnce(&Path) -> Result<BulkBuildReport, String>,
) -> Result<BulkBuildReport, String> {
    use std::os::unix::fs::DirBuilderExt;
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let parent = out
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let stage = loop {
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = parent.join(format!(".bulk-stage-{}-{n}", std::process::id()));
        if path == out {
            continue;
        }
        match fs::DirBuilder::new().mode(0o700).create(&path) {
            Ok(()) => break path,
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.to_string()),
        }
    };
    let result = (|| {
        let report = write(&stage)?;
        File::open(&stage)
            .and_then(|dir| dir.sync_all())
            .map_err(|e| e.to_string())?;
        if let Err(error) = fs::rename(&stage, out) {
            let manifest = JsonByteProof {
                bytes: fs::metadata(stage.join("manifest.json"))
                    .map_err(|e| e.to_string())?
                    .len(),
                sha256: report.manifest_sha256.clone(),
            };
            if !fs::symlink_metadata(out).is_ok_and(|m| m.is_dir())
                || !matches_payload(&out.join("manifest.json"), &manifest)
                || !matches_payload(&out.join("catalog.json"), &report.manifest.catalog)
                || !matches_payload(&out.join("index.json"), &report.manifest.index)
            {
                return Err(format!(
                    "cannot publish bulk artifact to {}: {error}",
                    out.display()
                ));
            }
        }
        Ok(report)
    })();
    match fs::remove_dir_all(&stage) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(format!("cannot remove bulk staging directory: {e}")),
    }
    result
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
        let header = include_str!("../../../tests/fixtures/archive-manifest-current.json")
            .as_bytes()
            .to_vec();
        let mut accepted = accepted_manifest(line_text(&header).unwrap())
            .unwrap()
            .clone();
        let raw =
            include_str!("../../../tests/fixtures/ramos/fd949f82-fc10-4e37-8aa9-6c7569fe3c55.json")
                .as_bytes();
        accepted.file_sha256 = Some(sha256(&[header.as_slice(), raw].concat()));
        let (catalog, index, _) = intake(Cursor::new(raw), header.clone(), &accepted).unwrap();
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

    #[test]
    fn stage1d_bulk_publication_keeps_occupied_receipts_and_cleans_failed_stage() {
        let parent = std::env::temp_dir().join(format!("mtg-bulk-publish-{}", std::process::id()));
        fs::create_dir(&parent).unwrap();
        let dest = parent.join("bulk");
        let write = |stage: &Path| {
            let proof = |file: &str| {
                write_payload(&stage.join(file), 64, |w| {
                    w.write_all(file.as_bytes()).map_err(|e| e.to_string())
                })
            };
            let catalog = proof("catalog.json")?;
            let index = proof("index.json")?;
            let manifest_proof = proof("manifest.json")?;
            Ok(BulkBuildReport {
                manifest_sha256: manifest_proof.sha256,
                manifest: BulkManifest {
                    format_version: BULK_FORMAT_VERSION,
                    ir_version: CURRENT_IR_VERSION,
                    rules_edition: RULES_EDITION.into(),
                    rules_sha256: RULES_SHA256.into(),
                    producer_build_id: String::new(),
                    archive_sha256: String::new(),
                    archive_manifest_sha256: String::new(),
                    parser_revision: String::new(),
                    parser_source_sha256: String::new(),
                    catalog_digest: catalog.sha256.clone(),
                    catalog,
                    index,
                    outcome_counts: BTreeMap::new(),
                },
            })
        };
        publish(&dest, write).unwrap();
        publish(&dest, write).unwrap();
        fs::write(dest.join("index.json"), b"preserve").unwrap();
        assert!(
            publish(&dest, write)
                .unwrap_err()
                .contains("cannot publish")
        );
        assert_eq!(fs::read(dest.join("index.json")).unwrap(), b"preserve");
        assert_eq!(fs::read_dir(&parent).unwrap().count(), 1);
        fs::remove_dir_all(parent).unwrap();
    }
}
