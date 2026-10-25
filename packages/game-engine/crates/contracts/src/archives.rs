//! Accepted source identities are reviewed data, shared by lowering and IR validation.
use crate::{sha256, strict_read};
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcceptedArchive {
    pub manifest_file: String,
    pub manifest_sha256: String,
    /// Historical full archive bytes were not retained; its committed records
    /// remain pinned individually. Extraction requires a known full-file hash.
    pub file_sha256: Option<String>,
    pub parser_revision: String,
    pub parser_source_sha256: String,
    pub corpus_sha256: String,
    /// Reviewed raw-record hashes bound to this origin, independent of the
    /// mutable archive filename on a provenance row. Extraction extends these
    /// only after verifying the retained full archive hash and original bytes.
    pub record_sha256: std::collections::BTreeSet<String>,
}
static ARCHIVES: LazyLock<Result<Vec<AcceptedArchive>, String>> = LazyLock::new(|| {
    let archives: Vec<AcceptedArchive> = strict_read(include_str!(
        "../../../tests/fixtures/accepted-archives.json"
    ))?;
    let mut identities = std::collections::BTreeSet::new();
    for a in &archives {
        for h in [
            &a.manifest_sha256,
            &a.parser_source_sha256,
            &a.corpus_sha256,
        ]
        .into_iter()
        .chain(a.file_sha256.iter())
        .chain(a.record_sha256.iter())
        {
            if h.len() != 64
                || !h
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
            {
                return Err("invalid accepted archive SHA-256".into());
            }
        }
        if !identities.insert(&a.manifest_sha256) {
            return Err("duplicate accepted archive identity".into());
        }
    }
    Ok(archives)
});
pub fn accepted_archives() -> Result<&'static [AcceptedArchive], String> {
    ARCHIVES.as_ref().map(Vec::as_slice).map_err(Clone::clone)
}
pub fn accepted_manifest(manifest: &str) -> Result<&'static AcceptedArchive, String> {
    let m: serde_json::Value = strict_read(manifest)?;
    let hash = sha256(manifest.trim_end().as_bytes());
    let a = accepted_archives()?
        .iter()
        .find(|a| a.manifest_sha256 == hash)
        .ok_or("unsupported archive version, source identity, or rules edition")?;
    if m["recordType"] != "manifest"
        || m["artifactKind"] != "oracle-ast-archive"
        || m["schemaVersion"] != 3
        || m["executable"] != false
        || m["parser"]["revision"] != a.parser_revision
        || m["parser"]["sourceSha256"] != a.parser_source_sha256
        || m["corpus"]["sha256"] != a.corpus_sha256
        || m["rules"]["edition"] != crate::RULES_EDITION
    {
        return Err("accepted archive metadata mismatch".into());
    }
    Ok(a)
}
pub fn accepted_provenance(manifest_sha256: &str, parser_sha256: &str) -> bool {
    accepted_archives().is_ok_and(|archives| {
        archives.iter().any(|a| {
            a.manifest_sha256 == manifest_sha256 && a.parser_source_sha256 == parser_sha256
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accepted_archives_require_the_matching_parser_and_exact_header() {
        for header in [
            include_str!("../../../tests/fixtures/archive-manifest.json"),
            include_str!("../../../tests/fixtures/archive-manifest-current.json"),
        ] {
            let a = accepted_manifest(header).unwrap();
            assert!(accepted_provenance(
                &a.manifest_sha256,
                &a.parser_source_sha256
            ));
            assert!(!accepted_provenance(&a.manifest_sha256, &"0".repeat(64)));
            let mut m: serde_json::Value = strict_read(header).unwrap();
            m["parser"]["revision"] = "unaccepted".into();
            assert!(accepted_manifest(&m.to_string()).is_err());
        }
    }
}
