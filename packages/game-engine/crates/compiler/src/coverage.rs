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
    for prefix in ["unknown variant `", "unknown field `"] {
        if let Some(rest) = reason.strip_prefix(prefix)
            && let Some((value, _)) = rest.split_once('`')
        {
            fn tag(v: &Value, value: &str) -> Option<String> {
                match v {
                    Value::Array(a) => a.iter().find_map(|v| tag(v, value)),
                    Value::Object(o) => {
                        for key in [
                            "kind", "trigger", "effect", "keyword", "amount", "ref", "cost",
                            "player", "modifier",
                        ] {
                            if o.get(key).and_then(Value::as_str) == Some(value) {
                                return Some(format!("{key}:{value}"));
                            }
                        }
                        o.values().find_map(|v| tag(v, value))
                    }
                    _ => None,
                }
            }
            return tag(abilities, value).unwrap_or_else(|| format!("{path}.{value}"));
        }
    }
    if reason.starts_with("unsupported complete keyword shape") {
        fn keyword(v: &Value) -> Option<String> {
            match v {
                Value::Array(a) => a.iter().find_map(keyword),
                Value::Object(o) => {
                    if let Some(k) = o.get("keyword").and_then(Value::as_str) {
                        let allowed = match k {
                            "equip" => &["keyword", "cost"][..],
                            "enchant" => &["keyword", "filter"][..],
                            _ => &["keyword"][..],
                        };
                        if o.len() != allowed.len()
                            || o.keys().any(|k| !allowed.contains(&k.as_str()))
                        {
                            return Some(format!("keyword:{k}"));
                        }
                    }
                    o.values().find_map(keyword)
                }
                _ => None,
            }
        }
        if let Some(k) = keyword(abilities) {
            return k;
        }
    }
    if reason.starts_with("multi_face_layout:") || reason.starts_with("keyword:") {
        return reason.into();
    }
    let detail = reason.split(" at line ").next().unwrap_or(reason);
    let slug = detail
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("_")
        .to_lowercase();
    format!("{path}:{slug}")
}
fn escape_tsv(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('\t', "\\t")
        .replace('\r', "\\r")
        .replace('\n', "\\n")
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
        ir_version: 20,
        total: 0,
        commander_legal: 0,
        lowered_validated: 0,
        lowered_validated_percent: 0.0,
        statuses: BTreeMap::new(),
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
        let (status, reason) = match compile(header, raw) {
            CompileResult::Lowered { definition } => match definition.validate() {
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
