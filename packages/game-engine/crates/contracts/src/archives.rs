//! Accepted source identities are reviewed data, shared by lowering and IR validation.
use crate::{sha256, strict_read};
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ArchiveCut {
    R4,
    R5,
    R6,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct R6Pins {
    pub file_bytes: u64,
    pub max_line_bytes: usize,
    pub input_records: u32,
    pub canonical_records_sha256: String,
    pub printed_schema_sha256: String,
    pub printed_vocabulary_sha256: String,
    pub printed_generator_sha256: String,
    pub printed_records_sha256: String,
    pub name_catalog_sha256: String,
    pub name_catalog_names: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcceptedArchive {
    pub cut: ArchiveCut,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub pending: bool,
    /// A measured cut whose producer is still under review. It authorizes
    /// archive intake and comparison but no tracked fixture rows, so no
    /// default consumer can select it before promotion clears the flag.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub candidate: bool,
    pub manifest_file: String,
    pub manifest_sha256: String,
    /// Historical full archive bytes were not retained; its committed records
    /// remain pinned individually. Extraction requires a known full-file hash.
    pub file_sha256: Option<String>,
    pub parser_revision: String,
    pub parser_source_sha256: String,
    pub corpus_sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r6: Option<R6Pins>,
    /// Reviewed raw-record hashes bound to this origin, independent of the
    /// mutable archive filename on a provenance row. Extraction extends these
    /// only after verifying the retained full archive hash and original bytes.
    pub record_sha256: std::collections::BTreeSet<String>,
}
static ARCHIVES: LazyLock<Result<Vec<AcceptedArchive>, String>> = LazyLock::new(|| {
    validate_registry(strict_read(include_str!(
        "../../../tests/fixtures/accepted-archives.json"
    ))?)
});

fn validate_registry(archives: Vec<AcceptedArchive>) -> Result<Vec<AcceptedArchive>, String> {
    let mut identities = std::collections::BTreeSet::new();
    for a in &archives {
        if a.candidate && (a.cut != ArchiveCut::R6 || a.pending || !a.record_sha256.is_empty()) {
            return Err("a candidate cut must be a measured R6 cut without record pins".into());
        }
        match (a.cut, a.pending, &a.r6) {
            (ArchiveCut::R6, true, None)
                if a.file_sha256.is_none() && a.record_sha256.is_empty() => {}
            // Bulk v2 carries the registered reviewed producer commit, never
            // the compiling checkout's revision.
            (ArchiveCut::R6, false, Some(pins))
                if a.file_sha256.is_some()
                    && a.parser_revision.len() == 40
                    && a.parser_revision
                        .bytes()
                        .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
                    && (1..=crate::bulk_catalog::ARCHIVE_MAX_BYTES).contains(&pins.file_bytes)
                    && (1..=crate::bulk_catalog::ARCHIVE_MAX_LINE_BYTES)
                        .contains(&pins.max_line_bytes)
                    && (1..=crate::bulk_catalog::ARCHIVE_MAX_RECORDS)
                        .contains(&(pins.input_records as usize))
                    && pins.name_catalog_names > 0
                    && [
                        &pins.canonical_records_sha256,
                        &pins.printed_schema_sha256,
                        &pins.printed_vocabulary_sha256,
                        &pins.printed_generator_sha256,
                        &pins.printed_records_sha256,
                        &pins.name_catalog_sha256,
                    ]
                    .into_iter()
                    .all(|hash| crate::bulk_catalog::is_sha256(hash)) => {}
            (ArchiveCut::R4 | ArchiveCut::R5, false, None) => {}
            _ => return Err("invalid archive cut registration".into()),
        }
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
}
pub fn accepted_archives() -> Result<&'static [AcceptedArchive], String> {
    ARCHIVES.as_ref().map(Vec::as_slice).map_err(Clone::clone)
}
pub fn accepted_manifest(manifest: &str) -> Result<&'static AcceptedArchive, String> {
    let m: serde_json::Value = strict_read(manifest)?;
    let hash = sha256(manifest.trim_end().as_bytes());
    let a = accepted_archives()?
        .iter()
        .find(|a| !a.pending && a.manifest_sha256 == hash)
        .ok_or("unsupported archive version, source identity, or rules edition")?;
    if a.cut == ArchiveCut::R6
        && sha256(manifest.strip_suffix('\n').unwrap_or(manifest).as_bytes()) != hash
    {
        return Err("R6 header bytes differ from the registered header".into());
    }
    let r5 = accepted_archives()?
        .iter()
        .find(|a| a.cut == ArchiveCut::R5);
    validate_header(&m, a, r5)?;
    Ok(a)
}

fn validate_header(
    m: &serde_json::Value,
    a: &AcceptedArchive,
    r5: Option<&AcceptedArchive>,
) -> Result<(), String> {
    if m["recordType"] != "manifest"
        || m["artifactKind"] != "oracle-ast-archive"
        || m["executable"] != false
        || m["parser"]["sourceSha256"] != a.parser_source_sha256
        || m["corpus"]["sha256"] != a.corpus_sha256
        || m["rules"]["edition"] != crate::RULES_EDITION
    {
        return Err("accepted archive metadata mismatch".into());
    }
    match a.cut {
        ArchiveCut::R4 | ArchiveCut::R5
            if m["schemaVersion"] == 4 && m["parser"]["revision"] == a.parser_revision =>
        {
            Ok(())
        }
        ArchiveCut::R6 if !a.pending => {
            let pins = a.r6.as_ref().ok_or("R6 cut has no measured pins")?;
            let legacy = r5.ok_or("R6 cut has no R5 baseline")?;
            if m["schemaVersion"] == 5
                && m["cut"] == "r6"
                && m["parser"]
                    .as_object()
                    .is_some_and(|parser| parser.len() == 1)
                && m["rules"]["sha256"] == crate::RULES_SHA256
                && m["printed"]["schemaVersion"] == 1
                && m["printed"]["schemaSha256"] == pins.printed_schema_sha256
                && m["printed"]["vocabularySha256"] == pins.printed_vocabulary_sha256
                && m["printed"]["generatorSha256"] == pins.printed_generator_sha256
                && m["printed"]["recordsSha256"] == pins.printed_records_sha256
                && m["nameCatalog"]["sha256"] == pins.name_catalog_sha256
                && m["nameCatalog"]["names"].as_u64() == Some(pins.name_catalog_names as u64)
                && m["legacy"]["manifestSha256"] == legacy.manifest_sha256
                && m["legacy"]["fileSha256"].as_str() == legacy.file_sha256.as_deref()
                && m["inputRecords"] == pins.input_records
                && m["corpus"]["canonicalRecordsSha256"] == pins.canonical_records_sha256
                && m["corpus"]["sha256"] == legacy.corpus_sha256
                && m["printedSummary"].is_object()
            {
                Ok(())
            } else {
                Err("accepted R6 archive metadata mismatch".into())
            }
        }
        _ => Err("archive cut and schema mismatch".into()),
    }
}
pub fn accepted_provenance(manifest_sha256: &str, parser_sha256: &str) -> bool {
    accepted_archives().is_ok_and(|archives| {
        archives.iter().any(|a| {
            !a.pending
                && a.manifest_sha256 == manifest_sha256
                && a.parser_source_sha256 == parser_sha256
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accepted_archives_require_the_matching_parser_and_exact_header() {
        {
            let header = include_str!("../../../tests/fixtures/archive-manifest-phase1-r5.json");
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

    #[test]
    fn the_promoted_r6_accepts_its_exact_header_and_record_pins() {
        let r6 = accepted_archives()
            .unwrap()
            .iter()
            .find(|entry| entry.cut == ArchiveCut::R6)
            .unwrap();
        assert!(!r6.candidate && !r6.pending && !r6.record_sha256.is_empty());
        let header = include_str!(
            "../../../tests/fixtures/archive-manifest-r6-candidate-8d421c9a7463061f2e5803d4d9a7df7860919b77ceb7fdd9732901ca298b4093.json"
        );
        assert_eq!(accepted_manifest(header).unwrap(), r6);
        assert!(accepted_provenance(
            &r6.manifest_sha256,
            &r6.parser_source_sha256
        ));
        let mut extra_lf = header.to_owned();
        extra_lf.push('\n');
        assert!(accepted_manifest(&extra_lf).is_err());

        let registry = accepted_archives().unwrap().to_vec();
        assert!(validate_registry(registry.clone()).is_ok());
        let edit = |change: &dyn Fn(&mut AcceptedArchive)| {
            let mut registry = registry.clone();
            change(
                registry
                    .iter_mut()
                    .find(|a| a.cut == ArchiveCut::R6)
                    .unwrap(),
            );
            validate_registry(registry)
        };
        assert!(edit(&|a| a.candidate = true).is_err());
        assert!(
            edit(&|a| {
                a.candidate = true;
                a.record_sha256.clear();
            })
            .is_ok()
        );
        assert!(edit(&|a| a.parser_revision.clear()).is_err());
        assert!(edit(&|a| a.parser_revision = "A".repeat(40)).is_err());
        let mut r5 = registry.clone();
        r5.iter_mut()
            .find(|a| a.cut == ArchiveCut::R5)
            .unwrap()
            .candidate = true;
        assert!(validate_registry(r5).is_err());
    }

    #[test]
    fn measured_r6_uses_source_identity_and_the_pending_entry_cannot_authorize_rows() {
        let legacy = accepted_archives()
            .unwrap()
            .iter()
            .find(|entry| entry.cut == ArchiveCut::R5)
            .unwrap();
        let mut r6 = accepted_archives()
            .unwrap()
            .iter()
            .find(|entry| entry.cut == ArchiveCut::R6)
            .unwrap()
            .clone();
        if r6.pending {
            assert!(!accepted_provenance(
                &r6.manifest_sha256,
                &r6.parser_source_sha256
            ));
        }
        let digest = "1".repeat(64);
        r6.pending = false;
        r6.parser_revision = "2".repeat(40);
        r6.parser_source_sha256 = digest.clone();
        r6.r6 = Some(R6Pins {
            file_bytes: 1,
            max_line_bytes: 1,
            input_records: 1,
            canonical_records_sha256: digest.clone(),
            printed_schema_sha256: digest.clone(),
            printed_vocabulary_sha256: digest.clone(),
            printed_generator_sha256: digest.clone(),
            printed_records_sha256: digest.clone(),
            name_catalog_sha256: digest.clone(),
            name_catalog_names: 1,
        });
        let mut header = serde_json::json!({
            "recordType": "manifest", "artifactKind": "oracle-ast-archive", "cut": "r6",
            "schemaVersion": 5, "executable": false, "inputRecords": 1,
            "corpus": {"sha256": legacy.corpus_sha256, "canonicalRecordsSha256": digest},
            "parser": {"sourceSha256": digest},
            "rules": {"edition": crate::RULES_EDITION, "sha256": crate::RULES_SHA256},
            "printed": {"schemaVersion": 1, "schemaSha256": digest, "vocabularySha256": digest,
                "generatorSha256": digest, "recordsSha256": digest},
            "nameCatalog": {"sha256": digest, "names": 1}, "printedSummary": {},
            "legacy": {"manifestSha256": legacy.manifest_sha256, "fileSha256": legacy.file_sha256},
        });
        assert!(validate_header(&header, &r6, Some(legacy)).is_ok());
        assert!(accepted_manifest(&header.to_string()).is_err());
        header["parser"]["revision"] = r6.parser_revision.clone().into();
        assert!(validate_header(&header, &r6, Some(legacy)).is_err());
        header["schemaVersion"] = 4.into();
        assert!(validate_header(&header, &r6, Some(legacy)).is_err());
    }
}
