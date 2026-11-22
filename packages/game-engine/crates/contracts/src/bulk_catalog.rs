//! Pinned offline bulk catalog format. Validation uses typed values and byte proofs.
use crate::{
    CURRENT_IR_VERSION, Catalog, Color, Definition, JsonByteProof, RULES_EDITION, RULES_SHA256,
    Sha256Sink, accepted_archives, strict_read,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeMap, io::Write, sync::Arc};

pub const BULK_FORMAT_VERSION: u32 = 2;
pub const ARCHIVE_MAX_BYTES: u64 = 1024 * 1024 * 1024;
pub const ARCHIVE_MAX_LINE_BYTES: usize = 1024 * 1024;
pub const ARCHIVE_MAX_RECORDS: usize = 60_000;
pub const MANIFEST_MAX_BYTES: u64 = 64 * 1024;
pub const CATALOG_MAX_BYTES: u64 = 128 * 1024 * 1024;
pub const INDEX_MAX_BYTES: u64 = 32 * 1024 * 1024;
pub const REASON_MAX_BYTES: usize = 256;

pub fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(lower_hex)
}

fn lower_hex(b: u8) -> bool {
    b.is_ascii_digit() || (b'a'..=b'f').contains(&b)
}

pub fn is_oracle_id(id: &str) -> bool {
    id.len() == 36
        && id.bytes().enumerate().all(|(i, b)| match i {
            8 | 13 | 18 | 23 => b == b'-',
            _ => lower_hex(b),
        })
}

/// Match the ordering and encoding of canonical(&Catalog) with only one
/// definition's intermediate JSON Value alive at a time.
pub fn write_catalog_canonical(catalog: &Catalog, mut output: impl Write) -> Result<(), String> {
    let write =
        |output: &mut dyn Write, bytes: &[u8]| output.write_all(bytes).map_err(|e| e.to_string());
    write(&mut output, br#"{"definitions":["#)?;
    for (index, definition) in catalog.definitions.iter().enumerate() {
        if index > 0 {
            write(&mut output, b",")?;
        }
        let value = serde_json::to_value(definition).map_err(|e| e.to_string())?;
        serde_json::to_writer(&mut output, &value).map_err(|e| e.to_string())?;
    }
    write(&mut output, br#"],"rules_edition":"#)?;
    serde_json::to_writer(&mut output, &catalog.rules_edition).map_err(|e| e.to_string())?;
    write(&mut output, br#","rules_sha256":"#)?;
    serde_json::to_writer(&mut output, &catalog.rules_sha256).map_err(|e| e.to_string())?;
    write(&mut output, br#","schema_version":"#)?;
    serde_json::to_writer(&mut output, &catalog.schema_version).map_err(|e| e.to_string())?;
    write(&mut output, b"}")
}

/// Immutable, validated IR and its canonical digest for sharing in-process.
#[derive(Clone, Debug)]
pub struct ValidatedCatalog {
    catalog: Arc<Catalog>,
    digest: String,
}
impl ValidatedCatalog {
    pub fn new(catalog: Catalog) -> Result<Self, String> {
        catalog.validate()?;
        let mut hash = Sha256Sink::new();
        write_catalog_canonical(&catalog, &mut hash)?;
        Ok(Self {
            catalog: Arc::new(catalog),
            digest: hash.finalize_hex(),
        })
    }
    pub fn catalog(&self) -> &Catalog {
        &self.catalog
    }
    pub fn shared_catalog(&self) -> Arc<Catalog> {
        Arc::clone(&self.catalog)
    }
    pub fn digest(&self) -> &str {
        &self.digest
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BulkStatus {
    LoweredValidated,
    Unsupported,
    Invalid,
    Unparsed,
}
impl BulkStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LoweredValidated => "lowered_validated",
            Self::Unsupported => "unsupported",
            Self::Invalid => "invalid",
            Self::Unparsed => "unparsed",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommanderLegality {
    Legal,
    NotLegal,
    Banned,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BulkCardFacts {
    pub name: String,
    pub commander_legality: CommanderLegality,
    pub conspiracy: bool,
    pub ante: bool,
    pub legendary_creature: bool,
    pub time_lord_doctor: bool,
    pub doctors_companion: bool,
    pub partner: bool,
    pub color_identity: Vec<Color>,
}
impl BulkCardFacts {
    pub fn from_source(source: &Value) -> Result<Self, String> {
        let name = source["name"]
            .as_str()
            .filter(|name| !name.is_empty() && name.len() <= 256)
            .ok_or("invalid bulk source name")?;
        let commander_legality = match source["legalities"]["commander"].as_str() {
            Some("legal") => CommanderLegality::Legal,
            Some("not_legal") => CommanderLegality::NotLegal,
            Some("banned") => CommanderLegality::Banned,
            _ => return Err("invalid source Commander legality".into()),
        };
        // Same finite-pool restrictions as embedded profile intake.
        let type_line = source["type_line"]
            .as_str()
            .ok_or("missing source type line")?;
        let conspiracy = type_line
            .split_whitespace()
            .any(|term| term == "Conspiracy");
        let oracle_text = source["oracle_text"].as_str().unwrap_or_default();
        let ante = oracle_text
            .split(|c: char| !c.is_alphabetic())
            .any(|term| term.eq_ignore_ascii_case("ante"));

        // CR 205.3m, 702.73, 702.124h/m and 903.3. Scryfall's keywords
        // also marks "Partner with" as Partner. The exact standalone Oracle
        // keyword line is the authority for these supported single-face forms.
        // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
        let (types, subtypes) = type_line.split_once(" — ").unwrap_or((type_line, ""));
        let single_face = !type_line.contains(" // ") && source["card_faces"].is_null();
        let legendary_creature = single_face
            && types.split_whitespace().any(|t| t == "Legendary")
            && types.split_whitespace().any(|t| t == "Creature");
        let changeling = source["keywords"]
            .as_array()
            .is_some_and(|keywords| keywords.iter().any(|keyword| keyword == "Changeling"));
        let all_types = oracle_text.lines().any(|line| {
            line.strip_prefix(name)
                .is_some_and(|rest| rest.starts_with(" is every creature type"))
        });
        let time_lord_doctor = legendary_creature
            && !changeling
            && !all_types
            && matches!(subtypes, "Time Lord Doctor" | "Doctor Time Lord");
        let doctors_companion = legendary_creature && oracle_text.lines().any(|line| {
            matches!(
                line,
                "Doctor's companion"
                    | "Doctor's companion (You can have two commanders if the other is the Doctor.)"
            )
        });
        let partner = legendary_creature
            && oracle_text.lines().any(|line| {
                matches!(
                    line,
                    "Partner" | "Partner (You can have two commanders if both have partner.)"
                )
            });
        let mut color_identity: Vec<Color> =
            serde_json::from_value(source["color_identity"].clone())
                .map_err(|_| "missing or invalid source color identity")?;
        if color_identity.contains(&Color::C) {
            return Err("colorless is not a color identity component".into());
        }
        color_identity.sort_unstable();
        color_identity.dedup();
        Ok(Self {
            name: name.into(),
            commander_legality,
            conspiracy,
            ante,
            legendary_creature,
            time_lord_doctor,
            doctors_companion,
            partner,
            color_identity,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BulkIndexRow {
    pub oracle_id: String,
    pub facts: BulkCardFacts,
    pub archive_line: usize,
    /// Exact input bytes, including a line terminator when present.
    pub raw_row_sha256: String,
    /// Existing fixture/reparse pin excluding the line terminator.
    pub record_sha256: String,
    pub source_index: u32,
    pub source_sha256: String,
    pub ast_sha256: Option<String>,
    pub binding_sha256: Option<String>,
    pub status: BulkStatus,
    pub reason_code: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BulkIndex {
    pub rows: Vec<BulkIndexRow>,
}
impl BulkIndex {
    pub fn lookup(&self, id: &str) -> Option<&BulkIndexRow> {
        self.rows
            .binary_search_by(|row| row.oracle_id.as_str().cmp(id))
            .ok()
            .map(|i| &self.rows[i])
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BulkManifest {
    pub format_version: u32,
    pub ir_version: u32,
    pub rules_edition: String,
    pub rules_sha256: String,
    pub producer_build_id: String,
    pub archive_sha256: String,
    pub archive_manifest_sha256: String,
    pub parser_revision: String,
    pub parser_source_sha256: String,
    pub catalog: JsonByteProof,
    pub index: JsonByteProof,
    pub catalog_digest: String,
    pub outcome_counts: BTreeMap<BulkStatus, usize>,
}
impl BulkManifest {
    pub fn validate(&self, expected_producer: &str) -> Result<(), String> {
        if self.format_version != BULK_FORMAT_VERSION
            || self.ir_version != CURRENT_IR_VERSION
            || self.rules_edition != RULES_EDITION
            || self.rules_sha256 != RULES_SHA256
            || !is_sha256(expected_producer)
            || self.producer_build_id != expected_producer
        {
            return Err("bulk version, rules or producer mismatch".into());
        }
        if !accepted_archives()?.iter().any(|archive| {
            archive.file_sha256.as_deref() == Some(self.archive_sha256.as_str())
                && archive.manifest_sha256 == self.archive_manifest_sha256
                && archive.parser_revision == self.parser_revision
                && archive.parser_source_sha256 == self.parser_source_sha256
        }) {
            return Err("bulk archive or parser identity is not accepted".into());
        }
        if !(1..=CATALOG_MAX_BYTES).contains(&self.catalog.bytes)
            || !(1..=INDEX_MAX_BYTES).contains(&self.index.bytes)
            || !is_sha256(&self.catalog.sha256)
            || !is_sha256(&self.index.sha256)
            || self.catalog.sha256 != self.catalog_digest
        {
            return Err("bulk payload proof is invalid".into());
        }
        Ok(())
    }
}

pub struct BulkArtifact {
    manifest_sha256: String,
    catalog: ValidatedCatalog,
    index: BulkIndex,
}
impl BulkArtifact {
    /// Native callers decode from bounded readers and provide both matching raw byte proofs.
    pub fn verify(
        (manifest, manifest_proof): (BulkManifest, JsonByteProof),
        expected_manifest: &str,
        expected_producer: &str,
        (catalog, catalog_proof): (Catalog, JsonByteProof),
        (index, index_proof): (BulkIndex, JsonByteProof),
    ) -> Result<Self, String> {
        if !(1..=MANIFEST_MAX_BYTES).contains(&manifest_proof.bytes)
            || !is_sha256(expected_manifest)
            || manifest_proof.sha256 != expected_manifest
        {
            return Err("bulk manifest pin mismatch".into());
        }
        manifest.validate(expected_producer)?;
        if manifest.catalog != catalog_proof || manifest.index != index_proof {
            return Err("bulk payload byte proof mismatch".into());
        }
        let catalog = ValidatedCatalog::new(catalog)?;
        if catalog.digest() != manifest.catalog_digest {
            return Err("bulk canonical catalog digest mismatch".into());
        }
        validate_index(&manifest, catalog.catalog(), &index)?;
        Ok(Self {
            manifest_sha256: manifest_proof.sha256,
            catalog,
            index,
        })
    }

    pub fn manifest_sha256(&self) -> &str {
        &self.manifest_sha256
    }
    pub fn catalog(&self) -> &ValidatedCatalog {
        &self.catalog
    }
    pub fn lookup(&self, id: &str) -> Option<&BulkIndexRow> {
        self.index.lookup(id)
    }
}

fn validate_definition(
    manifest: &BulkManifest,
    row: &BulkIndexRow,
    d: &Definition,
) -> Result<(), String> {
    let p = &d.provenance;
    let source: Value = strict_read(&p.source_json)?;
    if row.oracle_id != d.oracle_id
        || source["oracle_id"] != d.oracle_id
        || row.facts.name != d.name
        || row.facts != BulkCardFacts::from_source(&source)?
        || row.facts.color_identity.len() != d.characteristics.color_identity.len()
        || !row
            .facts
            .color_identity
            .iter()
            .all(|c| d.characteristics.color_identity.contains(c))
        || row.source_index != p.source_index
        || row.source_sha256 != p.source_sha256
        || row.ast_sha256.as_deref() != Some(p.ast_sha256.as_str())
        || row.binding_sha256.as_deref() != Some(p.binding_sha256.as_str())
        || p.archive_manifest_sha256 != manifest.archive_manifest_sha256
        || p.parser_sha256 != manifest.parser_source_sha256
    {
        return Err(format!(
            "bulk definition provenance mismatch for {}",
            row.oracle_id
        ));
    }
    Ok(())
}

pub fn validate_index(
    manifest: &BulkManifest,
    catalog: &Catalog,
    index: &BulkIndex,
) -> Result<(), String> {
    if !(1..=ARCHIVE_MAX_RECORDS).contains(&index.rows.len())
        || !index
            .rows
            .windows(2)
            .all(|r| r[0].oracle_id < r[1].oracle_id)
    {
        return Err("bulk index must contain unique, sorted Oracle identities".into());
    }
    let mut archive_lines = vec![false; index.rows.len()];
    let mut counts = BTreeMap::new();
    let mut definitions = catalog.definitions.iter();
    for row in &index.rows {
        let line = row
            .archive_line
            .checked_sub(2)
            .and_then(|i| archive_lines.get_mut(i));
        if line.is_none_or(|seen| std::mem::replace(seen, true))
            || !is_oracle_id(&row.oracle_id)
            || row.facts.name.is_empty()
            || row.facts.name.len() > 256
            || !is_sha256(&row.source_sha256)
            || !is_sha256(&row.raw_row_sha256)
            || !is_sha256(&row.record_sha256)
            || row
                .ast_sha256
                .iter()
                .chain(&row.binding_sha256)
                .any(|s| !is_sha256(s))
        {
            return Err("invalid bulk index row identity".into());
        }
        *counts.entry(row.status).or_insert(0) += 1;
        match (row.status, &row.reason_code) {
            (BulkStatus::LoweredValidated, None) => {
                let d = definitions
                    .next()
                    .ok_or("bulk index success has no definition")?;
                validate_definition(manifest, row, d)?;
            }
            (BulkStatus::LoweredValidated, Some(_)) => {
                return Err("bulk index success has a reason code".into());
            }
            (_, Some(reason)) if !reason.is_empty() && reason.len() <= REASON_MAX_BYTES => {}
            _ => return Err("bulk index gap has no bounded reason code".into()),
        }
    }
    if definitions.next().is_some() || counts != manifest.outcome_counts {
        return Err("bulk index and manifest outcomes differ".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{canonical, sha256};

    #[test]
    fn stage1d_streaming_bulk_catalog_matches_canonical_bytes_and_digest() {
        let mut catalog = Catalog::read(include_str!(
            "../../../tests/fixtures/expected-match-ir.json"
        ))
        .unwrap();
        for count in [catalog.definitions.len(), 2, 0] {
            catalog.definitions.truncate(count);
            let expected = canonical(&catalog).unwrap();
            let mut bytes = Vec::new();
            write_catalog_canonical(&catalog, &mut bytes).unwrap();
            assert_eq!(bytes, expected.as_bytes());
            let validated = ValidatedCatalog::new(catalog.clone()).unwrap();
            assert_eq!(validated.digest(), sha256(&bytes));
            assert!(Arc::ptr_eq(
                &validated.shared_catalog(),
                &validated.clone().shared_catalog()
            ));
        }
    }
}
