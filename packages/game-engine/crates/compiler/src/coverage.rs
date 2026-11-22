//! Reproducible whole-archive intake measurement; no header substitution.
use crate::{CompileResult, compile_result, lower};
use mtg_contracts::bulk_catalog::BulkStatus;
use mtg_contracts::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CoverageError {
    Io { reason: String },
    UnknownArchive { reason: String },
    ArchiveHashMismatch,
    InvalidRecord { line: usize, reason: String },
}
impl std::fmt::Display for CoverageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            serde_json::to_string(self).map_err(|_| std::fmt::Error)?
        )
    }
}
impl std::error::Error for CoverageError {}
fn io(e: std::io::Error) -> CoverageError {
    CoverageError::Io {
        reason: e.to_string(),
    }
}
#[derive(Debug, Serialize)]
pub struct ReasonCount {
    pub status: String,
    pub reason: String,
    pub count: usize,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub examples: Vec<String>,
}
#[derive(Debug, Serialize)]
pub struct CoverageSummary {
    pub report_version: u32,
    pub archive_sha256: String,
    pub archive_manifest_sha256: String,
    pub parser_revision: String,
    pub ir_version: u32,
    pub total: usize,
    pub commander_legal: usize,
    pub lowered_validated: usize,
    pub lowered_validated_percent: f64,
    pub statuses: BTreeMap<String, usize>,
    pub reasons: Vec<ReasonCount>,
    /// Present only for a selected-row run. Counts and records then cover the
    /// matched rows only; whole-archive runs omit the field.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selection: Option<CoverageSelection>,
}
/// Identity of an Oracle-ID selection. `oracle_ids_sha256` hashes the sorted,
/// distinct IDs, each followed by `\n`.
#[derive(Debug, Serialize)]
pub struct CoverageSelection {
    pub oracle_ids_sha256: String,
    pub requested: usize,
    pub matched: usize,
    pub missing: Vec<String>,
}
impl CoverageSelection {
    pub fn ids_sha256(ids: &BTreeSet<String>) -> String {
        sha256(
            ids.iter()
                .fold(String::new(), |out, id| out + id + "\n")
                .as_bytes(),
        )
    }
}
/// Machine codes do not contain serde line/column numbers or expected-variant
/// lists. Unknown discriminants retain their AST tag; other boundaries retain
/// their compiler field path and a stable, punctuation-free detail.
pub(crate) fn reason_code(path: &str, reason: &str, abilities: &Value) -> String {
    let reason = reason.split(" at line ").next().unwrap_or(reason);
    for prefix in ["unknown variant `", "unknown field `"] {
        if let Some(rest) = reason.strip_prefix(prefix)
            && let Some((value, _)) = rest.split_once('`')
        {
            // `abilities` here is the failed typed node, not a whole-card
            // search domain. Buffered enum locations are recovered by diagnostics.
            let tag = abilities.as_object().and_then(|o| {
                [
                    "kind",
                    "trigger",
                    "effect",
                    "keyword",
                    "amount",
                    "ref",
                    "cost",
                    "player",
                    "modifier",
                    "static",
                    "event",
                    "replacement",
                    "mana",
                    "shield",
                    "prevent",
                    "to",
                    "cls",
                ]
                .into_iter()
                .find(|key| o.get(*key).and_then(Value::as_str) == Some(value))
            });
            return tag
                .map(|key| format!("{key}:{value}"))
                .unwrap_or_else(|| format!("{}.{value}", stable_path(path)));
        }
    }
    if [
        "subtype:",
        "multi_face_layout:",
        "keyword:",
        "kind:",
        "trigger:",
        "effect:",
        "ref:",
        "amount:",
        "static:",
        "cost:",
        "player:",
        "modifier:",
        "event:",
        "replacement:",
        "mana:",
        "shield:",
        "prevent:",
        "to:",
        "cls:",
        "abilities[",
        "abilities.",
    ]
    .iter()
    .any(|prefix| reason.starts_with(prefix))
    {
        return reason.into();
    }
    let detail = reason.split(" at line ").next().unwrap_or(reason);
    let slug = detail
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("_")
        .to_lowercase();
    let path = stable_path(path);
    format!("{path}:{slug}")
}
pub(crate) fn stable_path(path: &str) -> String {
    let mut out = String::new();
    let mut inside = false;
    for c in path.chars() {
        if c == '[' {
            inside = true;
            out.push_str("[*]");
        } else if c == ']' {
            inside = false;
        } else if !inside {
            out.push(c);
        }
    }
    out
}
fn escape_tsv(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('\t', "\\t")
        .replace('\r', "\\r")
        .replace('\n', "\\n")
}

fn archive_exclusion(row: &Value) -> Option<String> {
    // Excluded archive records have no faces, binding or AST. They are valid
    // archive rows but were never parsed, including one Commander-legal row.
    (row["status"] == "excluded").then(|| {
        format!(
            "archive_excluded:{}",
            row["reason"].as_str().unwrap_or("missing_reason")
        )
    })
}

pub(crate) enum RecordAssessment {
    Definition(Definition),
    Gap { status: BulkStatus, reason: String },
}

pub(crate) fn assess_record(
    accepted: &AcceptedArchive,
    raw: &str,
    row: &Value,
) -> RecordAssessment {
    let gap = |status, reason| RecordAssessment::Gap { status, reason };
    if let Some(reason) = archive_exclusion(row) {
        return gap(BulkStatus::Unparsed, reason);
    }
    let ast = &row["faces"][0]["result"]["abilities"];
    match compile_result(lower(accepted, raw)) {
        CompileResult::Lowered { definition } => {
            let mut singleton = Catalog {
                schema_version: CURRENT_IR_VERSION,
                rules_edition: RULES_EDITION.into(),
                rules_sha256: RULES_SHA256.into(),
                definitions: vec![*definition],
            };
            match singleton.validate() {
                Ok(()) => RecordAssessment::Definition(singleton.definitions.pop().unwrap()),
                Err(e) => gap(BulkStatus::Invalid, reason_code("ir", &e, ast)),
            }
        }
        CompileResult::Unsupported { path, reason: _ } if path == "status" => {
            gap(BulkStatus::Unparsed, "status:unparsed".into())
        }
        CompileResult::Unsupported { path, reason } => {
            gap(BulkStatus::Unsupported, reason_code(&path, &reason, ast))
        }
        CompileResult::Invalid { path, reason } => {
            gap(BulkStatus::Invalid, reason_code(&path, &reason, ast))
        }
    }
}

/// One record that neither a family nor the compositional pipeline lowers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompositionGap {
    pub oracle_id: String,
    pub name: String,
    pub commander_legal: bool,
    /// The first blocker once card-wide binding checks are deferred: the
    /// block table row (cost, event, step, instruction or matrix cell) that is
    /// missing, or the binding check itself when every block would lower.
    pub reason: String,
    /// The card-wide reference-graph rejection, when the archived binding
    /// also fails. Lowering enforces it before any block.
    pub binding: Option<String>,
    /// The printed block `reason` names, when it is a per-block blocker.
    pub block: Option<usize>,
}

/// Diagnostic only: for every record that neither a family nor the
/// compositional pipeline lowers, every blocker composition names. Lowering
/// still rejects at the first card-wide binding failure; this report defers
/// those checks so a downstream block blocker is not hidden behind them.
pub fn composition_gaps(archive: &Path) -> Result<Vec<CompositionGap>, CoverageError> {
    let bytes = fs::read(archive).map_err(io)?;
    let text = std::str::from_utf8(&bytes).map_err(|e| CoverageError::InvalidRecord {
        line: 1,
        reason: e.to_string(),
    })?;
    let header = text
        .lines()
        .next()
        .ok_or_else(|| CoverageError::UnknownArchive {
            reason: "empty archive".into(),
        })?;
    let accepted =
        accepted_manifest(header).map_err(|reason| CoverageError::UnknownArchive { reason })?;
    if accepted.file_sha256.as_deref() != Some(sha256(&bytes).as_str()) {
        return Err(CoverageError::ArchiveHashMismatch);
    }
    let mut gaps = vec![];
    for (i, raw) in text.lines().skip(1).enumerate() {
        let row: Value = strict_read(raw).map_err(|reason| CoverageError::InvalidRecord {
            line: i + 2,
            reason,
        })?;
        if archive_exclusion(&row).is_some() || crate::lower_families(accepted, raw).is_ok() {
            continue;
        }
        let found = crate::compose::blockers(accepted, raw);
        let abilities = &row["faces"][0]["result"]["abilities"];
        let code = |f: &crate::Failure| reason_code(&f.path, &f.reason, abilities);
        let binding = found.binding.as_ref().map(code);
        let Some(reason) = found
            .lowering
            .as_ref()
            .map(code)
            .or_else(|| binding.clone())
        else {
            continue;
        };
        gaps.push(CompositionGap {
            oracle_id: row["oracleIdentity"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
            name: row["source"]["name"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
            commander_legal: row["source"]["legalities"]["commander"] == "legal",
            reason,
            binding,
            block: found.block,
        });
    }
    Ok(gaps)
}

pub fn coverage(archive: &Path, out: &Path) -> Result<CoverageSummary, CoverageError> {
    coverage_selected(archive, out, None)
}

/// First `oracleIdentity` value in a row without parsing it. JSON escapes
/// quotes inside strings, so the pattern only matches an object key; escaped
/// values fall back to a full parse. Selected rows are re-checked strictly.
fn row_identity(raw: &str) -> Option<std::borrow::Cow<'_, str>> {
    const KEY: &str = "\"oracleIdentity\"";
    let quick = raw.find(KEY).and_then(|at| {
        let rest = raw[at + KEY.len()..].trim_start().strip_prefix(':')?;
        let rest = rest.trim_start().strip_prefix('"')?;
        let id = &rest[..rest.find('"')?];
        (!id.contains('\\')).then_some(id)
    });
    if quick.is_some() {
        return quick.map(Into::into);
    }
    #[derive(Deserialize)]
    struct RowIdentity {
        #[serde(rename = "oracleIdentity")]
        oracle_identity: Option<String>,
    }
    serde_json::from_str::<RowIdentity>(raw)
        .ok()?
        .oracle_identity
        .map(Into::into)
}

/// Assess only rows whose `oracleIdentity` is selected. The whole archive is
/// still hashed against its accepted manifest, so accepted bytes are pinned;
/// unselected rows are neither lowered nor strictly re-read.
pub fn coverage_selected(
    archive: &Path,
    out: &Path,
    only: Option<&BTreeSet<String>>,
) -> Result<CoverageSummary, CoverageError> {
    let bytes = fs::read(archive).map_err(io)?;
    let text = std::str::from_utf8(&bytes).map_err(|e| CoverageError::InvalidRecord {
        line: 1,
        reason: e.to_string(),
    })?;
    let header = text
        .lines()
        .next()
        .ok_or_else(|| CoverageError::UnknownArchive {
            reason: "empty archive".into(),
        })?;
    let accepted =
        accepted_manifest(header).map_err(|reason| CoverageError::UnknownArchive { reason })?;
    let hash = sha256(&bytes);
    if accepted.file_sha256.as_deref() != Some(hash.as_str()) {
        return Err(CoverageError::ArchiveHashMismatch);
    }
    let mut summary = CoverageSummary {
        report_version: 1,
        archive_sha256: hash,
        archive_manifest_sha256: accepted.manifest_sha256.clone(),
        parser_revision: accepted.parser_revision.clone(),
        ir_version: mtg_contracts::CURRENT_IR_VERSION,
        total: 0,
        commander_legal: 0,
        lowered_validated: 0,
        lowered_validated_percent: 0.0,
        statuses: ["lowered_validated", "unsupported", "invalid", "unparsed"]
            .into_iter()
            .map(|s| (s.to_owned(), 0))
            .collect(),
        reasons: vec![],
        selection: None,
    };
    let mut matched = BTreeSet::new();
    let mut reasons: BTreeMap<(String, String), (usize, Vec<String>)> = BTreeMap::new();
    let mut tsv = String::from("oracle_id\tname\tstatus\treason\n");
    for (i, raw) in text.lines().skip(1).enumerate() {
        let selected = match (only, row_identity(raw)) {
            (None, _) => None,
            (Some(only), Some(id)) if only.contains(id.as_ref()) => Some(id),
            (Some(_), _) => continue,
        };
        let row: Value = strict_read(raw).map_err(|reason| CoverageError::InvalidRecord {
            line: i + 2,
            reason,
        })?;
        if row["recordType"] != "card" {
            return Err(CoverageError::InvalidRecord {
                line: i + 2,
                reason: "expected card record".into(),
            });
        }
        let id = row["oracleIdentity"]
            .as_str()
            .ok_or_else(|| CoverageError::InvalidRecord {
                line: i + 2,
                reason: "missing oracleIdentity".into(),
            })?;
        if let Some(selected) = selected
            && (selected != id || !matched.insert(id.to_owned()))
        {
            return Err(CoverageError::InvalidRecord {
                line: i + 2,
                reason: "selected oracleIdentity is not the unique top-level key".into(),
            });
        }
        let name = row["source"]["name"]
            .as_str()
            .ok_or_else(|| CoverageError::InvalidRecord {
                line: i + 2,
                reason: "missing source.name".into(),
            })?;
        let legal = row["source"]["legalities"]["commander"]
            .as_str()
            .ok_or_else(|| CoverageError::InvalidRecord {
                line: i + 2,
                reason: "missing source.legalities.commander".into(),
            })?
            == "legal";
        let result = assess_record(accepted, raw, &row);
        let (status, reason) = match result {
            RecordAssessment::Definition(_) => {
                (BulkStatus::LoweredValidated.as_str(), String::new())
            }
            RecordAssessment::Gap { status, reason } => (status.as_str(), reason),
        };
        summary.total += 1;
        if legal {
            summary.commander_legal += 1;
            *summary.statuses.entry(status.into()).or_default() += 1;
            if status == "lowered_validated" {
                summary.lowered_validated += 1;
            } else {
                let (count, examples) = reasons.entry((status.into(), reason.clone())).or_default();
                *count += 1;
                examples.push(name.to_owned());
            }
        }
        tsv.push_str(&format!(
            "{}\t{}\t{status}\t{}\n",
            escape_tsv(id),
            escape_tsv(name),
            escape_tsv(&reason)
        ));
    }
    if let Some(only) = only {
        summary.selection = Some(CoverageSelection {
            oracle_ids_sha256: CoverageSelection::ids_sha256(only),
            requested: only.len(),
            matched: matched.len(),
            missing: only.difference(&matched).cloned().collect(),
        });
    }
    summary.lowered_validated_percent = if summary.commander_legal == 0 {
        0.0
    } else {
        100.0 * summary.lowered_validated as f64 / summary.commander_legal as f64
    };
    summary.reasons = reasons
        .into_iter()
        .map(|((status, reason), (count, mut examples))| {
            examples.sort();
            examples.dedup();
            examples.truncate(5);
            ReasonCount {
                status,
                reason,
                count,
                examples,
            }
        })
        .collect();
    summary.reasons.sort_by(|a, b| {
        b.count
            .cmp(&a.count)
            .then(a.reason.cmp(&b.reason))
            .then(a.status.cmp(&b.status))
    });
    for r in summary.reasons.iter_mut().skip(30) {
        r.examples.clear();
    }
    let json = format!(
        "{}\n",
        serde_json::to_string_pretty(&summary).expect("serializable summary")
    );
    publish_receipts(out, &tsv, &json).map_err(io)?;
    Ok(summary)
}

fn publish_receipts(out: &Path, records: &str, summary: &str) -> std::io::Result<()> {
    use std::io::{Error, ErrorKind};
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);

    let files = [("records.tsv", records), ("summary.json", summary)];
    match fs::symlink_metadata(out) {
        Ok(meta) if meta.is_dir() && fs::read_dir(out)?.next().is_none() => {}
        Ok(meta) => {
            if meta.is_dir()
                && files.iter().all(|(name, contents)| {
                    let path = out.join(name);
                    fs::symlink_metadata(&path).is_ok_and(|m| m.is_file())
                        && fs::read(path).is_ok_and(|bytes| bytes == contents.as_bytes())
                })
            {
                return Ok(());
            }
            return Err(Error::new(
                ErrorKind::AlreadyExists,
                format!(
                    "coverage output {} is occupied; choose a fresh directory",
                    out.display()
                ),
            ));
        }
        Err(e) if e.kind() == ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }

    let parent = out
        .parent()
        .ok_or_else(|| Error::other("missing output parent"))?;
    fs::create_dir_all(parent)?;
    let stage = loop {
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = parent.join(format!(".coverage-stage-{}-{id}", std::process::id()));
        if path == out {
            continue;
        }
        match fs::create_dir(&path) {
            Ok(()) => break path,
            Err(e) if e.kind() == ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    };
    // Sibling directories share a filesystem. A competing complete publication
    // makes the destination nonempty, so rename cannot overwrite its contents.
    let result = (|| {
        for (name, contents) in files {
            fs::write(stage.join(name), contents)?;
        }
        fs::rename(&stage, out)
    })();
    if result.is_err() {
        fs::remove_dir_all(stage)?;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn receipt_publication_preserves_prior_and_unrelated_output() {
        let dir = std::env::temp_dir().join(format!("coverage-publication-{}", std::process::id()));
        fs::create_dir(&dir).unwrap();
        let out = dir.join("output");
        publish_receipts(&out, "first records", "first summary").unwrap();
        publish_receipts(&out, "first records", "first summary").unwrap();
        assert!(publish_receipts(&out, "second records", "second summary").is_err());
        assert_eq!(
            fs::read_to_string(out.join("records.tsv")).unwrap(),
            "first records"
        );
        assert_eq!(
            fs::read_to_string(out.join("summary.json")).unwrap(),
            "first summary"
        );
        fs::remove_file(out.join("records.tsv")).unwrap();
        fs::write(out.join("notes"), "keep").unwrap();
        assert!(publish_receipts(&out, "second records", "second summary").is_err());
        assert!(!out.join("records.tsv").exists());
        assert_eq!(fs::read_to_string(out.join("notes")).unwrap(), "keep");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn row_identity_matches_keys_not_escaped_text() {
        let row =
            r#"{"recordType":"card","text":"\"oracleIdentity\":\"x\"","oracleIdentity" : "a-1"}"#;
        assert_eq!(row_identity(row).as_deref(), Some("a-1"));
        let escaped = r#"{"oracleIdentity":"a\u002d1"}"#;
        assert_eq!(row_identity(escaped).as_deref(), Some("a-1"));
        assert_eq!(row_identity(r#"{"recordType":"card"}"#), None);
        let ids: BTreeSet<String> = ["b".into(), "a".into()].into();
        assert_eq!(CoverageSelection::ids_sha256(&ids), sha256(b"a\nb\n"));
    }

    #[test]
    fn excluded_archive_rows_are_unparsed_not_malformed_compiler_records() {
        let row = serde_json::json!({"status":"excluded", "reason":"no-eligible-paper-printing"});
        assert_eq!(
            archive_exclusion(&row),
            Some("archive_excluded:no-eligible-paper-printing".into())
        );
        assert_eq!(
            archive_exclusion(&serde_json::json!({"status":"parsed"})),
            None
        );
    }
    #[test]
    fn archive_acceptance_and_raw_bytes_are_required_before_outputs() {
        let dir = std::env::temp_dir().join(format!("mtg-coverage-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let archive = dir.join("input.jsonl");
        let out = dir.join("out");
        fs::write(&archive, "{}\n").unwrap();
        assert!(matches!(
            coverage(&archive, &out),
            Err(CoverageError::UnknownArchive { .. })
        ));
        assert!(!out.exists());
        fs::write(
            &archive,
            include_str!("../../../tests/fixtures/archive-manifest-current.json"),
        )
        .unwrap();
        assert!(matches!(
            coverage(&archive, &out),
            Err(CoverageError::ArchiveHashMismatch)
        ));
        assert!(!out.exists());
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn reason_codes_do_not_depend_on_serde_locations_or_array_offsets() {
        let ast = serde_json::json!({"trigger":"phase"});
        assert_eq!(
            reason_code(
                "abilities[2]",
                "unknown variant `phase`, expected `enters` at line 1 column 19",
                &ast
            ),
            "trigger:phase"
        );
        assert_eq!(
            reason_code(
                "abilities[12]",
                "unknown field `condition` at line 1 column 99",
                &ast
            ),
            "abilities[*].condition"
        );
        assert_eq!(
            reason_code("abilities[1]", "keyword:ward at line 1 column 123", &ast),
            "keyword:ward"
        );
        assert_eq!(escape_tsv("A\tB\nC\\D"), "A\\tB\\nC\\\\D");
    }
}
