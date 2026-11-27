//! One frozen-input compile: a strict input manifest pins every input, the
//! accepted archive is read from a content-verified cache, and five files
//! publish in one stage (the three bulk format-2 files, `outcomes.jsonl` and
//! `compile-manifest.json`). Only the manifest's pinned content affects the
//! output bytes; manifest paths, the cache location and the checkout do not.
use crate::bulk::{
    IntakeError, OUTCOMES_MAX_BYTES, OutcomePhase, OutcomeRow, PublishError, StagedFiles,
    bulk_files, publish, read_archive, write_bulk, write_payload,
};
use mtg_contracts::{bulk_catalog::*, *};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{self, BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
};

/// The located failure classes of a compile. `Display` prefixes the class.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CompileError {
    /// The frozen manifest or a pinned source file.
    Input(String),
    /// The archive identity is not the registered one the pins name.
    Registration(String),
    /// The cached reader output (accepted archive) and its intake.
    Reader(String),
    /// This compiler's identity or catalog validation.
    Compiler(String),
    /// Staging or publishing the output bundle.
    Destination(String),
}
impl CompileError {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Input(_) => "input",
            Self::Registration(_) => "registration",
            Self::Reader(_) => "reader",
            Self::Compiler(_) => "compiler",
            Self::Destination(_) => "destination",
        }
    }
    pub fn reason(&self) -> &str {
        match self {
            Self::Input(e)
            | Self::Registration(e)
            | Self::Reader(e)
            | Self::Compiler(e)
            | Self::Destination(e) => e,
        }
    }
}
impl std::fmt::Display for CompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.kind(), self.reason())
    }
}
impl std::error::Error for CompileError {}

pub const INPUTS_ARTIFACT_KIND: &str = "oracle-compile-inputs";
pub const INPUTS_SCHEMA_VERSION: u32 = 1;
pub const BUNDLE_ARTIFACT_KIND: &str = "oracle-compile-bundle";
pub const BUNDLE_FORMAT_VERSION: u32 = 1;
const INPUTS_MAX_BYTES: u64 = 64 * 1024;
/// The only archive schema with a cached reader path in this compiler.
const CACHED_ARCHIVE_SCHEMA: u32 = 4;

/// Every object of the frozen manifest and its closed key set. Each key is a
/// mandatory pin except those in [`OPTIONAL_PINS`].
const INPUT_KEYS: &[(&str, &[&str])] = &[
    (
        "",
        &[
            "artifactKind",
            "schemaVersion",
            "oracle",
            "paperEvidence",
            "scope",
            "rules",
            "reader",
            "archive",
            "compiler",
        ],
    ),
    (
        "/oracle",
        &["path", "bytes", "sha256", "canonicalRecordsSha256"],
    ),
    ("/paperEvidence", &["path", "bytes", "sha256"]),
    ("/rules", &["edition", "sha256"]),
    ("/reader", &["revision", "sourceSha256"]),
    (
        "/archive",
        &["file", "schemaVersion", "bytes", "sha256", "headerSha256"],
    ),
    ("/compiler", &["irVersion", "bulkFormatVersion", "buildId"]),
];
/// Pinning the producer inside a tracked manifest is circular, so it is the
/// only optional pin. When present it must equal this binary's identity.
const OPTIONAL_PINS: &[&str] = &["/compiler/buildId"];
/// Location-only fields: they resolve bytes but never enter logical identity.
const LOCATION_FIELDS: &[&str] = &["/oracle/path", "/paperEvidence/path", "/archive/file"];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FrozenInputs {
    pub artifact_kind: String,
    pub schema_version: u32,
    pub oracle: OraclePin,
    pub paper_evidence: FilePin,
    /// The archive's paper scope and exclusion derivation.
    pub scope: String,
    pub rules: RulesPin,
    pub reader: ReaderPin,
    pub archive: ArchivePin,
    pub compiler: CompilerPin,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OraclePin {
    /// Relative to the manifest's directory.
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
    pub canonical_records_sha256: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FilePin {
    /// Relative to the manifest's directory.
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RulesPin {
    pub edition: String,
    pub sha256: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReaderPin {
    pub revision: String,
    pub source_sha256: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ArchivePin {
    /// File name of the reader output inside the cache directory.
    pub file: String,
    pub schema_version: u32,
    pub bytes: u64,
    pub sha256: String,
    /// SHA-256 of the header line without its terminator, as registered.
    pub header_sha256: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompilerPin {
    pub ir_version: u32,
    pub bulk_format_version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build_id: Option<String>,
}

fn input(e: impl Into<String>) -> CompileError {
    CompileError::Input(e.into())
}

/// Check the closed key set before typed decoding, so a missing pin and an
/// extra unpinned input each fail by pointer.
fn check_keys(manifest: &Value) -> Result<(), CompileError> {
    for (pointer, keys) in INPUT_KEYS {
        let object = manifest
            .pointer(pointer)
            .and_then(Value::as_object)
            .ok_or_else(|| input(format!("pin `{pointer}` must be an object")))?;
        if let Some(extra) = object.keys().find(|k| !keys.contains(&k.as_str())) {
            return Err(input(format!("unpinned input `{pointer}/{extra}`")));
        }
        for key in *keys {
            let at = format!("{pointer}/{key}");
            if object.get(*key).is_none_or(Value::is_null) && !OPTIONAL_PINS.contains(&at.as_str())
            {
                return Err(input(format!("missing pin `{at}`")));
            }
            if object.get(*key).is_some_and(Value::is_null) {
                return Err(input(format!("null pin `{at}`")));
            }
        }
    }
    Ok(())
}

fn relative(path: &str, pointer: &str) -> Result<(), CompileError> {
    if path.is_empty() || Path::new(path).is_absolute() {
        return Err(input(format!(
            "`{pointer}` must be a nonempty relative path"
        )));
    }
    Ok(())
}

impl FrozenInputs {
    pub fn read(json: &str) -> Result<Self, CompileError> {
        let manifest: Value = strict_read(json).map_err(|e| input(format!("manifest: {e}")))?;
        check_keys(&manifest)?;
        let inputs: Self =
            serde_json::from_value(manifest).map_err(|e| input(format!("manifest: {e}")))?;
        inputs.validate()?;
        Ok(inputs)
    }

    fn validate(&self) -> Result<(), CompileError> {
        if self.artifact_kind != INPUTS_ARTIFACT_KIND
            || self.schema_version != INPUTS_SCHEMA_VERSION
        {
            return Err(input(format!(
                "manifest must be {INPUTS_ARTIFACT_KIND} schema {INPUTS_SCHEMA_VERSION}"
            )));
        }
        for (pointer, value) in [
            ("/oracle/sha256", &self.oracle.sha256),
            (
                "/oracle/canonicalRecordsSha256",
                &self.oracle.canonical_records_sha256,
            ),
            ("/paperEvidence/sha256", &self.paper_evidence.sha256),
            ("/rules/sha256", &self.rules.sha256),
            ("/reader/sourceSha256", &self.reader.source_sha256),
            ("/archive/sha256", &self.archive.sha256),
            ("/archive/headerSha256", &self.archive.header_sha256),
        ]
        .into_iter()
        .chain(
            self.compiler
                .build_id
                .iter()
                .map(|id| ("/compiler/buildId", id)),
        ) {
            if !is_sha256(value) {
                return Err(input(format!("`{pointer}` is not a lowercase SHA-256")));
            }
        }
        relative(&self.oracle.path, "/oracle/path")?;
        relative(&self.paper_evidence.path, "/paperEvidence/path")?;
        let file = &self.archive.file;
        if file.is_empty() || file.contains('/') || file == "." || file == ".." {
            return Err(input("`/archive/file` must be a plain cache file name"));
        }
        if [
            self.oracle.bytes,
            self.paper_evidence.bytes,
            self.archive.bytes,
        ]
        .contains(&0)
            || self.scope.is_empty()
            || self.reader.revision.is_empty()
        {
            return Err(input(
                "byte lengths, scope and reader revision must be nonempty",
            ));
        }
        Ok(())
    }

    /// Content pins only: location fields and the optional producer pin are
    /// removed, so cache and checkout placement never enter identity.
    pub fn logical(&self) -> Value {
        let mut value = serde_json::to_value(self).expect("serializable inputs");
        for pointer in LOCATION_FIELDS.iter().chain(OPTIONAL_PINS) {
            let (parent, key) = pointer.rsplit_once('/').expect("pointer");
            if let Some(object) = value.pointer_mut(parent).and_then(Value::as_object_mut) {
                object.remove(key);
            }
        }
        value
    }
}

/// One verified input file.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedFile {
    pub role: String,
    pub bytes: u64,
    pub sha256: String,
}

/// The `identity` report: the logical inputs, their digest and every
/// resolved input verified against its pin.
#[derive(Clone, Debug, Serialize)]
pub struct IdentityReport {
    pub status: &'static str,
    pub inputs: Value,
    pub inputs_sha256: String,
    pub producer_build_id: String,
    pub registered_archive: String,
    pub resolved: Vec<ResolvedFile>,
}

struct Resolved {
    inputs: FrozenInputs,
    archive: PathBuf,
    registered: &'static AcceptedArchive,
    files: Vec<ResolvedFile>,
}

fn regular_file(
    path: &Path,
    role: &str,
    fail: fn(String) -> CompileError,
) -> Result<u64, CompileError> {
    let metadata =
        fs::symlink_metadata(path).map_err(|e| fail(format!("{role} {}: {e}", path.display())))?;
    if !metadata.is_file() {
        // Frozen inputs are hard links or regular files, never symlinks.
        return Err(fail(format!(
            "{role} {} is not a regular file",
            path.display()
        )));
    }
    Ok(metadata.len())
}

fn hash_file(path: &Path, limit: u64) -> io::Result<(u64, String)> {
    let mut hash = Sha256Sink::new();
    let size = io::copy(&mut File::open(path)?.take(limit + 1), &mut hash)?;
    Ok((size, hash.finalize_hex()))
}

fn verify_file(
    path: &Path,
    role: &str,
    bytes: u64,
    sha: &str,
) -> Result<ResolvedFile, CompileError> {
    let length = regular_file(path, role, CompileError::Input)?;
    if length != bytes {
        return Err(input(format!(
            "{role} byte length mismatch: pinned {bytes}, found {length}"
        )));
    }
    let (size, found) = hash_file(path, bytes).map_err(|e| input(format!("{role}: {e}")))?;
    if size != bytes || found != sha {
        return Err(input(format!(
            "{role} sha256 mismatch: pinned {sha}, found {found}"
        )));
    }
    Ok(ResolvedFile {
        role: role.into(),
        bytes,
        sha256: found,
    })
}

fn check_compiler(inputs: &FrozenInputs, producer: &str) -> Result<(), CompileError> {
    let fail = |e: String| Err(CompileError::Compiler(e));
    if !is_sha256(producer) {
        return fail("invalid producer identity".into());
    }
    if inputs.compiler.ir_version != CURRENT_IR_VERSION
        || inputs.compiler.bulk_format_version != BULK_FORMAT_VERSION
    {
        return fail(format!(
            "pinned IR {} / bulk format {} differ from this compiler's {CURRENT_IR_VERSION} / {BULK_FORMAT_VERSION}",
            inputs.compiler.ir_version, inputs.compiler.bulk_format_version
        ));
    }
    if inputs.rules.edition != RULES_EDITION || inputs.rules.sha256 != RULES_SHA256 {
        return fail("pinned rules edition or SHA-256 differs from this compiler's".into());
    }
    if let Some(pinned) = &inputs.compiler.build_id
        && pinned != producer
    {
        return fail(format!(
            "pinned build {pinned} differs from this binary's {producer}"
        ));
    }
    Ok(())
}

fn check_registration(inputs: &FrozenInputs) -> Result<&'static AcceptedArchive, CompileError> {
    let fail = |e: String| CompileError::Registration(e);
    let archive = &inputs.archive;
    let registered = accepted_archives()
        .map_err(fail)?
        .iter()
        .find(|a| a.manifest_sha256 == archive.header_sha256)
        .ok_or_else(|| {
            fail(format!(
                "archive header {} is not registered",
                archive.header_sha256
            ))
        })?;
    if registered.file_sha256.as_deref() != Some(archive.sha256.as_str()) {
        return Err(fail(format!(
            "pinned archive sha256 {} is not the registered full-file pin",
            archive.sha256
        )));
    }
    if registered.parser_revision != inputs.reader.revision
        || registered.parser_source_sha256 != inputs.reader.source_sha256
        || registered.corpus_sha256 != inputs.oracle.sha256
    {
        return Err(fail(
            "pinned reader or Oracle identity differs from the registered archive".into(),
        ));
    }
    Ok(registered)
}

/// Read the archive header line and require it to agree with every pin.
fn check_header(path: &Path, inputs: &FrozenInputs) -> Result<(), CompileError> {
    let reader_error = |e: String| CompileError::Reader(e);
    let mut header = Vec::new();
    BufReader::new(File::open(path).map_err(|e| reader_error(e.to_string()))?)
        .take(ARCHIVE_MAX_LINE_BYTES as u64 + 1)
        .read_until(b'\n', &mut header)
        .map_err(|e| reader_error(e.to_string()))?;
    let text = std::str::from_utf8(&header)
        .map_err(|e| reader_error(format!("archive header: {e}")))?
        .trim_end();
    if sha256(text.as_bytes()) != inputs.archive.header_sha256 {
        return Err(reader_error(format!(
            "cached archive header is not the pinned {}",
            inputs.archive.header_sha256
        )));
    }
    let header: Value =
        strict_read(text).map_err(|e| reader_error(format!("archive header: {e}")))?;
    let pins = [
        ("/schemaVersion", Value::from(inputs.archive.schema_version)),
        ("/scope", inputs.scope.clone().into()),
        ("/corpus/sha256", inputs.oracle.sha256.clone().into()),
        ("/corpus/bytes", inputs.oracle.bytes.into()),
        (
            "/corpus/canonicalRecordsSha256",
            inputs.oracle.canonical_records_sha256.clone().into(),
        ),
        (
            "/paperEvidence/sha256",
            inputs.paper_evidence.sha256.clone().into(),
        ),
        ("/paperEvidence/bytes", inputs.paper_evidence.bytes.into()),
        ("/parser/revision", inputs.reader.revision.clone().into()),
        (
            "/parser/sourceSha256",
            inputs.reader.source_sha256.clone().into(),
        ),
        ("/rules/edition", inputs.rules.edition.clone().into()),
    ];
    if let Some((pointer, _)) = pins
        .iter()
        .find(|(pointer, pin)| header.pointer(pointer) != Some(pin))
    {
        return Err(input(format!(
            "pin for archive header `{pointer}` disagrees with the registered header"
        )));
    }
    Ok(())
}

fn resolve(
    inputs_path: &Path,
    cache: &Path,
    producer: &str,
    hash_archive: bool,
) -> Result<Resolved, CompileError> {
    let length = regular_file(inputs_path, "input manifest", CompileError::Input)?;
    if length > INPUTS_MAX_BYTES {
        return Err(input("input manifest exceeds 64 KiB"));
    }
    let text =
        fs::read_to_string(inputs_path).map_err(|e| input(format!("input manifest: {e}")))?;
    let inputs = FrozenInputs::read(&text)?;
    check_compiler(&inputs, producer)?;
    let base = inputs_path.parent().unwrap_or(Path::new("."));
    let mut files = vec![
        verify_file(
            &base.join(&inputs.oracle.path),
            "oracle",
            inputs.oracle.bytes,
            &inputs.oracle.sha256,
        )?,
        verify_file(
            &base.join(&inputs.paper_evidence.path),
            "paperEvidence",
            inputs.paper_evidence.bytes,
            &inputs.paper_evidence.sha256,
        )?,
    ];
    let registered = check_registration(&inputs)?;
    if inputs.archive.schema_version != CACHED_ARCHIVE_SCHEMA {
        return Err(CompileError::Reader(format!(
            "archive schema {} has no cached reader in this compiler",
            inputs.archive.schema_version
        )));
    }
    let archive = cache.join(&inputs.archive.file);
    let length = regular_file(&archive, "cached archive", CompileError::Reader)?;
    if length != inputs.archive.bytes {
        return Err(CompileError::Reader(format!(
            "cached archive byte length mismatch: pinned {}, found {length}",
            inputs.archive.bytes
        )));
    }
    check_header(&archive, &inputs)?;
    if hash_archive {
        let (size, found) = hash_file(&archive, inputs.archive.bytes)
            .map_err(|e| CompileError::Reader(e.to_string()))?;
        if size != inputs.archive.bytes || found != inputs.archive.sha256 {
            return Err(CompileError::Reader(format!(
                "cached archive sha256 mismatch: pinned {}, found {found}",
                inputs.archive.sha256
            )));
        }
    }
    files.push(ResolvedFile {
        role: "archive".into(),
        bytes: inputs.archive.bytes,
        sha256: inputs.archive.sha256.clone(),
    });
    Ok(Resolved {
        inputs,
        archive,
        registered,
        files,
    })
}

/// Validate the manifest and verify every resolved input, including the
/// whole cached archive, without compiling.
pub fn identity(
    inputs_path: &Path,
    cache: &Path,
    producer: &str,
) -> Result<IdentityReport, CompileError> {
    let resolved = resolve(inputs_path, cache, producer, true)?;
    let inputs = resolved.inputs.logical();
    Ok(IdentityReport {
        status: "verified",
        inputs_sha256: digest(&inputs).map_err(CompileError::Compiler)?,
        inputs,
        producer_build_id: producer.into(),
        registered_archive: resolved.registered.manifest_file.clone(),
        resolved: resolved.files,
    })
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompileCounts {
    pub records: usize,
    pub definitions: usize,
    pub statuses: BTreeMap<BulkStatus, usize>,
    pub phases: BTreeMap<OutcomePhase, usize>,
}

/// `compile-manifest.json`: the logical frozen inputs, the verified producer,
/// versions, counts, and the exact length and hash of the other four files.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompileManifest {
    pub artifact_kind: String,
    pub format_version: u32,
    pub inputs: Value,
    pub inputs_sha256: String,
    pub producer_build_id: String,
    pub ir_version: u32,
    pub bulk_format_version: u32,
    pub counts: CompileCounts,
    pub files: BTreeMap<String, JsonByteProof>,
}

#[derive(Debug, Serialize)]
pub struct CompileReport {
    pub out: String,
    pub compile_manifest_sha256: String,
    pub compile_manifest: CompileManifest,
}

/// Every outcome joins its index row on identity, source, line, status and reason.
fn validate_outcomes(outcomes: &[OutcomeRow], index: &BulkIndex) -> Result<(), String> {
    if outcomes.len() != index.rows.len()
        || !outcomes.iter().zip(&index.rows).all(|(o, r)| {
            o.oracle_id == r.oracle_id
                && o.source_index == r.source_index
                && o.archive_line == r.archive_line
                && o.status == r.status
                && o.reason_code == r.reason_code
                && (o.status == BulkStatus::LoweredValidated)
                    == (o.phase == OutcomePhase::Validate && o.reason_code.is_none())
        })
    {
        return Err("outcomes do not join the bulk index".into());
    }
    Ok(())
}

fn read_back<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
    strict_read(&fs::read_to_string(path).map_err(|e| e.to_string())?)
}

/// Compile the frozen inputs into a new five-file bundle at `out`. An
/// existing `out` succeeds only when it already holds exactly this bundle.
pub fn compile(
    inputs_path: &Path,
    cache: &Path,
    out: &Path,
    producer: &str,
) -> Result<CompileReport, CompileError> {
    let resolved = resolve(inputs_path, cache, producer, false)?;
    let intake = read_archive(&resolved.archive).map_err(|e| match e {
        IntakeError::Reader(e) => CompileError::Reader(e),
        IntakeError::Compiler(e) => CompileError::Compiler(e),
    })?;
    if !std::ptr::eq(intake.identity, resolved.registered) {
        return Err(CompileError::Reader(
            "cached archive header changed during compilation".into(),
        ));
    }
    let compiler = |e: String| CompileError::Compiler(e);
    let destination = |e: String| CompileError::Destination(e);
    let inputs = resolved.inputs.logical();
    let inputs_sha256 = digest(&inputs).map_err(compiler)?;
    publish(out, true, |stage| {
        let bulk = write_bulk(stage, &intake, producer).map_err(compiler)?;
        let mut files: StagedFiles = bulk_files(stage, &bulk).map_err(destination)?;
        validate_outcomes(&intake.outcomes, &intake.index).map_err(compiler)?;
        let outcomes = write_payload(&stage.join("outcomes.jsonl"), OUTCOMES_MAX_BYTES, |w| {
            for row in &intake.outcomes {
                w.write_all(canonical(row)?.as_bytes())
                    .and_then(|()| w.write_all(b"\n"))
                    .map_err(|e| e.to_string())?;
            }
            Ok(())
        })
        .map_err(compiler)?;
        files.push(("outcomes.jsonl", outcomes));
        let mut phases = BTreeMap::new();
        for row in &intake.outcomes {
            *phases.entry(row.phase).or_insert(0) += 1;
        }
        let manifest = CompileManifest {
            artifact_kind: BUNDLE_ARTIFACT_KIND.into(),
            format_version: BUNDLE_FORMAT_VERSION,
            inputs: inputs.clone(),
            inputs_sha256: inputs_sha256.clone(),
            producer_build_id: producer.into(),
            ir_version: CURRENT_IR_VERSION,
            bulk_format_version: BULK_FORMAT_VERSION,
            counts: CompileCounts {
                records: intake.index.rows.len(),
                definitions: intake.catalog.definitions.len(),
                statuses: intake.counts.clone(),
                phases,
            },
            files: files
                .iter()
                .map(|(name, proof)| ((*name).to_owned(), proof.clone()))
                .collect(),
        };
        let proof = write_payload(
            &stage.join("compile-manifest.json"),
            MANIFEST_MAX_BYTES,
            |w| {
                w.write_all(canonical(&manifest)?.as_bytes())
                    .map_err(|e| e.to_string())
            },
        )
        .map_err(compiler)?;
        // Strict-read the staged structured files back before exposure; the
        // publisher then re-verifies every file's bytes against its proof.
        let staged = |name: &str| stage.join(name);
        let read_error = |e: String| CompileError::Destination(format!("staged bundle: {e}"));
        if read_back::<BulkManifest>(&staged("manifest.json")).map_err(read_error)? != bulk.manifest
            || read_back::<CompileManifest>(&staged("compile-manifest.json")).map_err(read_error)?
                != manifest
            || read_back::<BulkIndex>(&staged("index.json")).map_err(read_error)? != intake.index
        {
            return Err(destination("staged bundle does not read back".into()));
        }
        let outcomes_text =
            fs::read_to_string(staged("outcomes.jsonl")).map_err(|e| destination(e.to_string()))?;
        let reread = outcomes_text
            .lines()
            .map(strict_read::<OutcomeRow>)
            .collect::<Result<Vec<_>, _>>()
            .map_err(read_error)?;
        if reread != intake.outcomes {
            return Err(destination("staged outcomes do not read back".into()));
        }
        files.push(("compile-manifest.json", proof.clone()));
        Ok((
            CompileReport {
                out: out.display().to_string(),
                compile_manifest_sha256: proof.sha256,
                compile_manifest: manifest,
            },
            files,
        ))
    })
    .map_err(|e| match e {
        PublishError::Write(e) => e,
        PublishError::Destination(e) => CompileError::Destination(e),
    })
}

#[cfg(test)]
mod tests;
