//! Reproducible whole-archive intake measurement; no header substitution.
use crate::{CompileResult, compile};
use mtg_contracts::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeMap, fs, path::Path};

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
fn stable_path(path: &str) -> String {
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

fn archive_exclusion(row: &Value) -> Option<(&'static str, String)> {
    // Excluded archive records have no faces, binding or AST. They are valid
    // archive rows but were never parsed, including one Commander-legal row.
    (row["status"] == "excluded").then(|| {
        (
            "unparsed",
            format!(
                "archive_excluded:{}",
                row["reason"].as_str().unwrap_or("missing_reason")
            ),
        )
    })
}

pub fn coverage(archive: &Path, out: &Path) -> Result<CoverageSummary, CoverageError> {
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
        ir_version: 23,
        total: 0,
        commander_legal: 0,
        lowered_validated: 0,
        lowered_validated_percent: 0.0,
        statuses: ["lowered_validated", "unsupported", "invalid", "unparsed"]
            .into_iter()
            .map(|s| (s.to_owned(), 0))
            .collect(),
        reasons: vec![],
    };
    let mut reasons: BTreeMap<(String, String), (usize, Vec<String>)> = BTreeMap::new();
    let mut tsv = String::from("oracle_id\tname\tstatus\treason\n");
    for (i, raw) in text.lines().skip(1).enumerate() {
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
        let ast = &row["faces"][0]["result"]["abilities"];
        let (status, reason) = if let Some(excluded) = archive_exclusion(&row) {
            excluded
        } else {
            match compile(header, raw) {
                CompileResult::Lowered { definition } => match (Catalog {
                    schema_version: 23,
                    rules_edition: RULES_EDITION.into(),
                    rules_sha256: RULES_SHA256.into(),
                    definitions: vec![*definition],
                })
                .validate()
                {
                    Ok(()) => ("lowered_validated", String::new()),
                    Err(e) => ("invalid", reason_code("ir", &e, ast)),
                },
                CompileResult::Unsupported { path, reason: _ } if path == "status" => {
                    ("unparsed", "status:unparsed".into())
                }
                CompileResult::Unsupported { path, reason } => {
                    ("unsupported", reason_code(&path, &reason, ast))
                }
                CompileResult::Invalid { path, reason } => {
                    ("invalid", reason_code(&path, &reason, ast))
                }
            }
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
    fs::create_dir_all(out).map_err(io)?;
    fs::write(out.join("records.tsv"), tsv).map_err(io)?;
    fs::write(
        out.join("summary.json"),
        format!(
            "{}\n",
            serde_json::to_string_pretty(&summary).expect("serializable summary")
        ),
    )
    .map_err(io)?;
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn excluded_archive_rows_are_unparsed_not_malformed_compiler_records() {
        let row = serde_json::json!({"status":"excluded", "reason":"no-eligible-paper-printing"});
        assert_eq!(
            archive_exclusion(&row),
            Some((
                "unparsed",
                "archive_excluded:no-eligible-paper-printing".into()
            ))
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
