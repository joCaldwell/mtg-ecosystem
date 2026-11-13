//! Offline data-manifest catalog construction. No profile admission lives here.
use crate::{CompileResult, compile, fixtures};
use mtg_contracts::*;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Component, Path, PathBuf},
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CatalogError {
    InvalidManifest { path: PathBuf, reason: String },
    InvalidRecordGroups { reason: String },
    InvalidData { reason: String },
}
impl From<String> for CatalogError {
    fn from(reason: String) -> Self {
        Self::InvalidData { reason }
    }
}
impl From<&str> for CatalogError {
    fn from(reason: &str) -> Self {
        reason.to_owned().into()
    }
}
impl std::fmt::Display for CatalogError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            serde_json::to_string(self).map_err(|_| std::fmt::Error)?
        )
    }
}
impl std::error::Error for CatalogError {}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogManifest {
    pub schema_version: u32,
    pub output_order: OutputOrder,
    pub provenance: String,
    pub records: Vec<String>,
    pub expected_ir: String,
    pub expected_digest: String,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputOrder {
    Sorted,
    Struct,
    Canonical,
}
fn expected_json(manifest: &CatalogManifest, catalog: &Catalog) -> Result<String, String> {
    let json = match manifest.output_order {
        OutputOrder::Sorted => {
            serde_json::to_string_pretty(&serde_json::to_value(catalog).map_err(|e| e.to_string())?)
        }
        OutputOrder::Struct => serde_json::to_string_pretty(catalog),
        OutputOrder::Canonical => return Ok(format!("{}\n", canonical(catalog)?)),
    }
    .map_err(|e| e.to_string())?;
    Ok(format!("{json}\n"))
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FixtureProvenance {
    pub fixture_version: u32,
    pub rules_edition: String,
    pub rules_sha256: String,
    pub records: Vec<FixtureRecord>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FixtureRecord {
    pub file: String,
    pub oracle_id: String,
    pub record_sha256: String,
    pub source_sha256: String,
    pub ast_sha256: Option<String>,
    pub archive: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archive_line: Option<usize>,
}
fn relative(base: &Path, name: &str) -> PathBuf {
    let mut out = PathBuf::new();
    for c in base.join(name).components() {
        match c {
            Component::ParentDir if out.file_name().is_some_and(|n| n != "..") => {
                out.pop();
            }
            Component::CurDir => {}
            _ => out.push(c.as_os_str()),
        }
    }
    out
}
fn read_file(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))
}
fn embedded_file(path: &Path) -> Result<String, String> {
    fixtures::FILES
        .iter()
        .find(|(name, _)| Path::new(name) == path)
        .map(|(_, raw)| (*raw).to_owned())
        .ok_or_else(|| format!("missing embedded data {}", path.display()))
}
fn origin_registry(raw: &str) -> Result<Vec<AcceptedArchive>, String> {
    let origins: Vec<AcceptedArchive> = strict_read(raw)?;
    let mut seen = BTreeSet::new();
    for origin in &origins {
        let mut metadata = origin.clone();
        metadata.record_sha256.clear();
        let matches = accepted_archives()?.iter().any(|accepted| {
            let mut accepted = accepted.clone();
            accepted.record_sha256.clear();
            accepted == metadata
        });
        if !matches || !seen.insert(&origin.manifest_sha256) {
            return Err("unaccepted or duplicate archive origin metadata".into());
        }
        for hash in &origin.record_sha256 {
            if hash.len() != 64
                || !hash
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
            {
                return Err("invalid origin record SHA-256".into());
            }
        }
    }
    Ok(origins)
}
pub fn validate_embedded_record_groups() -> Result<(), CatalogError> {
    let raw = embedded_file(Path::new("record-groups.json"))?;
    let _: BTreeMap<String, Vec<String>> =
        strict_read(&raw).map_err(|reason| CatalogError::InvalidRecordGroups { reason })?;
    if let Some(reason) = fixtures::RECORD_GROUP_ERROR {
        return Err(CatalogError::InvalidRecordGroups {
            reason: reason.into(),
        });
    }
    Ok(())
}
fn build_with(
    path: &Path,
    read: impl Fn(&Path) -> Result<String, String>,
) -> Result<(CatalogManifest, Catalog), CatalogError> {
    let manifest: CatalogManifest =
        strict_read(&read(path)?).map_err(|reason| CatalogError::InvalidManifest {
            path: path.to_owned(),
            reason,
        })?;
    let base = path.parent().ok_or("manifest has no parent")?;
    let provenance_path = relative(base, &manifest.provenance);
    let provenance: FixtureProvenance = strict_read(&read(&provenance_path)?)?;
    if provenance.fixture_version != 1
        || provenance.rules_edition != RULES_EDITION
        || provenance.rules_sha256 != RULES_SHA256
    {
        return Err("unsupported fixture provenance version or rules".into());
    }
    let source_dir = provenance_path.parent().ok_or("provenance has no parent")?;
    let origins = origin_registry(&read(&source_dir.join("accepted-archives.json"))?)?;
    let mut witnesses = std::collections::BTreeMap::new();
    for row in &provenance.records {
        if witnesses
            .insert(relative(source_dir, &row.file), row)
            .is_some()
        {
            return Err(format!("duplicate provenance file {}", row.file).into());
        }
    }
    let mut files = BTreeSet::new();
    let mut definitions = vec![];
    for file in &manifest.records {
        let p = relative(base, file);
        if !files.insert(p.clone()) {
            return Err(format!("duplicate catalog record {file}").into());
        }
        let row = witnesses
            .get(&p)
            .ok_or_else(|| format!("missing provenance for {file}"))?;
        let raw = read(&p)?;
        if sha256(raw.trim_end().as_bytes()) != row.record_sha256 {
            return Err(format!("{file}: record hash mismatch").into());
        }
        let header = read(&relative(source_dir, &row.archive))?;
        let archive = accepted_manifest(&header)?;
        if archive.manifest_file != row.archive {
            return Err(format!("{file}: archive name mismatch").into());
        }
        if !origins.iter().any(|a| {
            a.manifest_sha256 == archive.manifest_sha256
                && a.record_sha256.contains(&row.record_sha256)
        }) {
            return Err(format!(
                "{file}: record bytes are not pinned to the labelled archive origin"
            )
            .into());
        }
        match compile(&header, &raw) {
            CompileResult::Lowered { definition } => {
                if definition.oracle_id != row.oracle_id
                    || definition.provenance.source_sha256 != row.source_sha256
                    || Some(definition.provenance.ast_sha256.as_str()) != row.ast_sha256.as_deref()
                {
                    return Err(format!("{file}: provenance identity mismatch").into());
                }
                definitions.push(*definition);
            }
            result => return Err(format!("{file}: {result:?}").into()),
        }
    }
    definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    let catalog = Catalog {
        schema_version: manifest.schema_version,
        rules_edition: RULES_EDITION.into(),
        rules_sha256: RULES_SHA256.into(),
        definitions,
    };
    catalog.validate()?;
    Ok((manifest, catalog))
}
pub fn build_catalog(path: &Path) -> Result<Catalog, CatalogError> {
    build_with(path, read_file).map(|(_, c)| c)
}
pub fn embedded_catalog(name: &str) -> Result<Catalog, CatalogError> {
    validate_embedded_record_groups()?;
    build_with(
        &Path::new("catalogs").join(format!("{name}.json")),
        embedded_file,
    )
    .map(|(_, c)| c)
}
/// Without --write this is a check, including exact expected JSON bytes.
/// The digest hashes canonical IR; the manifest selects the fixture encoding.
pub fn generate_catalog(path: &Path, write: bool) -> Result<serde_json::Value, CatalogError> {
    let (manifest, catalog) = build_with(path, read_file)?;
    let base = path.parent().ok_or("manifest has no parent")?;
    let expected = relative(base, &manifest.expected_ir);
    let digest_path = relative(base, &manifest.expected_digest);
    let hash = digest(&catalog)?;
    let bytes = expected_json(&manifest, &catalog)?;
    if write {
        fs::write(&expected, &bytes).map_err(|e| e.to_string())?;
        fs::write(&digest_path, &hash).map_err(|e| e.to_string())?;
    } else if read_file(&expected)? != bytes || read_file(&digest_path)? != hash {
        return Err(format!(
            "generated catalog differs: {} ({} definitions, SHA-256 {hash}); use --write",
            path.display(),
            catalog.definitions.len()
        )
        .into());
    }
    Ok(
        serde_json::json!({"manifest":path,"definitions":catalog.definitions.len(),"schema_version":catalog.schema_version,"sha256":hash,"expected_ir":expected,"written":write}),
    )
}

/// Verify the full archive pin and compare every provenance row for this origin
/// to its original line, including whitespace and the line terminator. Other
/// accepted origins are reported separately, never claimed as verified.
pub fn verify_archive(
    archive_path: &Path,
    provenance_path: &Path,
) -> Result<serde_json::Value, CatalogError> {
    use std::io::{BufRead, BufReader};
    let mut reader = BufReader::new(fs::File::open(archive_path).map_err(|e| e.to_string())?);
    let mut header = Vec::new();
    reader
        .read_until(b'\n', &mut header)
        .map_err(|e| e.to_string())?;
    let accepted = accepted_manifest(std::str::from_utf8(&header).map_err(|e| e.to_string())?)?;
    verify_archive_with(reader, &header, accepted, provenance_path).map_err(Into::into)
}
fn verify_archive_with(
    mut reader: impl std::io::BufRead,
    header: &[u8],
    accepted: &AcceptedArchive,
    provenance_path: &Path,
) -> Result<serde_json::Value, String> {
    use sha2::{Digest, Sha256};
    let expected_hash = accepted
        .file_sha256
        .as_ref()
        .ok_or("verification requires a retained full archive file SHA-256")?;
    let dir = provenance_path.parent().ok_or("provenance has no parent")?;
    if fs::read(dir.join(&accepted.manifest_file)).map_err(|e| e.to_string())? != header {
        return Err("fixture manifest differs from raw archive header".into());
    }
    let provenance: FixtureProvenance = strict_read(&read_file(provenance_path)?)?;
    if provenance.fixture_version != 1
        || provenance.rules_edition != RULES_EDITION
        || provenance.rules_sha256 != RULES_SHA256
    {
        return Err("unsupported fixture provenance version or rules".into());
    }
    let origins = origin_registry(&read_file(&dir.join("accepted-archives.json"))?)?;
    let origin = origins
        .iter()
        .find(|a| a.manifest_sha256 == accepted.manifest_sha256)
        .ok_or("missing accepted archive origin")?;
    let mut rows = BTreeMap::new();
    for row in provenance
        .records
        .iter()
        .filter(|r| r.archive == accepted.manifest_file)
    {
        let number = row
            .archive_line
            .ok_or_else(|| format!("{}: missing archive_line", row.file))?;
        if number < 2 || rows.insert(number, row).is_some() {
            return Err("invalid or duplicate archive_line".into());
        }
        if !origin.record_sha256.contains(&row.record_sha256) {
            return Err(format!(
                "{}: record bytes are not pinned to the labelled archive origin",
                row.file
            ));
        }
    }
    if rows.is_empty() {
        return Err("no provenance records for this archive".into());
    }
    let count = rows.len();
    let mut hash = Sha256::new();
    hash.update(header);
    let mut line = Vec::new();
    let mut number = 1usize;
    loop {
        line.clear();
        if reader
            .read_until(b'\n', &mut line)
            .map_err(|e| e.to_string())?
            == 0
        {
            break;
        }
        number = number.checked_add(1).ok_or("archive line count overflow")?;
        hash.update(&line);
        if let Some(row) = rows.remove(&number) {
            let raw = fs::read(dir.join(&row.file)).map_err(|e| format!("{}: {e}", row.file))?;
            if raw != line {
                return Err(format!(
                    "{}: archive line {number} differs from fixture bytes",
                    row.file
                ));
            }
            let record: serde_json::Value =
                strict_read(std::str::from_utf8(&raw).map_err(|e| e.to_string())?)?;
            if sha256(
                std::str::from_utf8(&raw)
                    .map_err(|e| e.to_string())?
                    .trim_end()
                    .as_bytes(),
            ) != row.record_sha256
                || record["oracleIdentity"] != row.oracle_id
                || record["sourceSha256"] != row.source_sha256
                || record["astSha256"].as_str() != row.ast_sha256.as_deref()
            {
                return Err(format!("{}: provenance identity/hash mismatch", row.file));
            }
        }
    }
    if format!("{:x}", hash.finalize()) != *expected_hash {
        return Err("archive file SHA-256 mismatch".into());
    }
    if !rows.is_empty() {
        return Err("archive_line is beyond end of archive".into());
    }
    Ok(
        serde_json::json!({"archive_sha256":expected_hash,"manifest":accepted.manifest_file,"verified_records":count,"other_origin_records":provenance.records.len()-count}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_committed_catalogs_are_fresh() {
        for (name, _) in fixtures::FILES
            .iter()
            .filter(|(name, _)| name.starts_with("catalogs/") && name.ends_with(".json"))
        {
            let path = Path::new(name);
            let (manifest, catalog) = build_with(path, embedded_file).unwrap();
            let base = path.parent().unwrap();
            assert!(
                read_file(
                    &Path::new(env!("CARGO_MANIFEST_DIR"))
                        .join("../../tests/fixtures")
                        .join(relative(base, &manifest.expected_ir))
                )
                .unwrap()
                    == expected_json(&manifest, &catalog).unwrap(),
                "{name}: expected IR bytes differ"
            );
            assert_eq!(
                read_file(
                    &Path::new(env!("CARGO_MANIFEST_DIR"))
                        .join("../../tests/fixtures")
                        .join(relative(base, &manifest.expected_digest))
                )
                .unwrap(),
                digest(&catalog).unwrap(),
                "{name}"
            );
        }
    }
}

/// Select exact names or Oracle IDs, verify the entire accepted JSONL, then
/// copy its raw lines (including their original line terminators) unchanged.
pub fn extract_records(
    archive_path: &Path,
    provenance_path: &Path,
    selectors: &[String],
) -> Result<serde_json::Value, String> {
    use std::io::{BufRead, BufReader};
    if selectors.is_empty() {
        return Err("select at least one exact name or Oracle ID".into());
    }
    let mut reader = BufReader::new(fs::File::open(archive_path).map_err(|e| e.to_string())?);
    let mut line = Vec::new();
    reader
        .read_until(b'\n', &mut line)
        .map_err(|e| e.to_string())?;
    let header = std::str::from_utf8(&line).map_err(|e| e.to_string())?;
    let accepted = accepted_manifest(header)?;
    extract_verified(reader, line, accepted, provenance_path, selectors)
}
fn extract_verified(
    mut reader: impl std::io::BufRead,
    header_bytes: Vec<u8>,
    accepted: &AcceptedArchive,
    provenance_path: &Path,
    selectors: &[String],
) -> Result<serde_json::Value, String> {
    use sha2::{Digest, Sha256};
    let header = std::str::from_utf8(&header_bytes).map_err(|e| e.to_string())?;
    let mut line = header_bytes.clone();
    let expected_hash = accepted
        .file_sha256
        .as_ref()
        .ok_or("extraction requires a retained full archive file SHA-256")?;
    let mut hash = Sha256::new();
    hash.update(&line);
    let dir = provenance_path.parent().ok_or("provenance has no parent")?;
    if read_file(&dir.join(&accepted.manifest_file))? != header {
        return Err("fixture manifest differs from raw archive header".into());
    }
    let mut provenance: FixtureProvenance = strict_read(&read_file(provenance_path)?)?;
    let origins_path = dir.join("accepted-archives.json");
    let mut origins = origin_registry(&read_file(&origins_path)?)?;
    if provenance.fixture_version != 1
        || provenance.rules_edition != RULES_EDITION
        || provenance.rules_sha256 != RULES_SHA256
    {
        return Err("unsupported fixture provenance version or rules".into());
    }
    #[derive(Deserialize)]
    struct Identity {
        #[serde(rename = "oracleIdentity")]
        id: String,
        source: Named,
    }
    #[derive(Deserialize)]
    struct Named {
        name: String,
    }
    let mut selected = std::collections::BTreeMap::new();
    let mut hits = vec![0usize; selectors.len()];
    let mut number = 1usize;
    loop {
        line.clear();
        if reader
            .read_until(b'\n', &mut line)
            .map_err(|e| e.to_string())?
            == 0
        {
            break;
        }
        number += 1;
        hash.update(&line);
        let identity: Identity =
            serde_json::from_slice(&line).map_err(|e| format!("archive line {number}: {e}"))?;
        let mut matches = false;
        for (selector, count) in selectors.iter().zip(&mut hits) {
            if *selector == identity.id || *selector == identity.source.name {
                *count += 1;
                matches = true;
            }
        }
        if matches {
            // Oracle IDs provide collision-free fixture filenames regardless of
            // punctuation, Unicode, or duplicate human-readable names.
            if identity.id.len() != 36
                || !identity
                    .id
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit() || b == b'-')
            {
                return Err("invalid Oracle ID filename".into());
            }
            selected.insert(identity.id, (number, identity.source.name, line.clone()));
        }
    }
    if format!("{:x}", hash.finalize()) != *expected_hash {
        return Err("archive file SHA-256 mismatch; no fixtures written".into());
    }
    for (selector, count) in selectors.iter().zip(hits) {
        if count != 1 {
            return Err(format!(
                "selector {selector:?}: expected exactly one record, found {count}; no fixtures written"
            ));
        }
    }
    let mut outputs = vec![];
    let mut report = vec![];
    for (id, (line_number, name, raw)) in selected {
        let file = format!("{id}.json");
        if dir.join(&file).exists()
            || provenance.records.iter().any(|r| {
                r.file == file || (r.oracle_id == id && r.archive == accepted.manifest_file)
            })
        {
            return Err(format!(
                "fixture/provenance already exists for {id}; no fixtures written"
            ));
        }
        // Extraction preserves records even when their AST cannot lower. Its
        // shallow mirror checks source integrity without imposing compiler AST
        // or complete-card parsing requirements on record selection.
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct ExtractRecord {
            record_type: String,
            oracle_identity: String,
            source_sha256: String,
            source: Box<serde_json::value::RawValue>,
            binding_sha256: Option<String>,
            binding: Option<Box<serde_json::value::RawValue>>,
            ast_sha256: Option<String>,
        }
        let record: ExtractRecord = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
        // Excluded archive records retain only their source and exclusion
        // reason; binding exists only for in-scope records (data_schemas.md).
        let binding_valid = match (&record.binding, &record.binding_sha256) {
            (Some(binding), Some(hash)) => sha256(binding.get().as_bytes()) == *hash,
            (None, None) => true,
            _ => false,
        };
        if record.oracle_identity != id
            || record.record_type != "card"
            || sha256(record.source.get().as_bytes()) != record.source_sha256
            || !binding_valid
        {
            return Err(format!(
                "{id}: invalid record identity or source/binding hashes"
            ));
        }
        let ast_sha256 = record.ast_sha256;
        if let Some(expected_ast) = &ast_sha256 {
            // Use raw subvalues for the JS archive hash; never reserialize AST
            // objects into Rust's sorted-key representation.
            #[derive(Deserialize)]
            struct Ast {
                abilities: Box<serde_json::value::RawValue>,
            }
            #[derive(Deserialize)]
            struct AstFace {
                result: Box<serde_json::value::RawValue>,
            }
            #[derive(Deserialize)]
            struct AstRecord {
                faces: Vec<AstFace>,
            }
            let ast: AstRecord = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
            let abilities = ast
                .faces
                .iter()
                .map(|f| {
                    serde_json::from_str::<Ast>(f.result.get())
                        .map(|a| a.abilities.get().to_owned())
                        .map_err(|e| e.to_string())
                })
                .collect::<Result<Vec<_>, _>>()?;
            if sha256(format!("[{}]", abilities.join(",")).as_bytes()) != *expected_ast {
                return Err(format!("{id}: AST hash mismatch"));
            }
        }
        let record_hash = sha256(
            std::str::from_utf8(&raw)
                .map_err(|e| e.to_string())?
                .trim_end()
                .as_bytes(),
        );
        let origin = origins
            .iter_mut()
            .find(|a| a.manifest_sha256 == accepted.manifest_sha256)
            .ok_or("missing accepted archive origin")?;
        origin.record_sha256.insert(record_hash);
        provenance.records.push(FixtureRecord {
            file: file.clone(),
            oracle_id: id.clone(),
            record_sha256: sha256(
                std::str::from_utf8(&raw)
                    .map_err(|e| e.to_string())?
                    .trim_end()
                    .as_bytes(),
            ),
            source_sha256: record.source_sha256,
            ast_sha256,
            archive: accepted.manifest_file.clone(),
            archive_line: Some(line_number),
        });
        report.push(
            serde_json::json!({"name":name,"oracle_id":id,"archive_line":line_number,"file":file}),
        );
        outputs.push((dir.join(file), raw));
    }
    // All selectors, identities, hashes and collisions are checked before writes.
    for (path, bytes) in outputs {
        fs::write(path, bytes).map_err(|e| e.to_string())?;
    }
    fs::write(
        &origins_path,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&origins).map_err(|e| e.to_string())?
        ),
    )
    .map_err(|e| e.to_string())?;
    fs::write(
        provenance_path,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&provenance).map_err(|e| e.to_string())?
        ),
    )
    .map_err(|e| e.to_string())?;
    Ok(serde_json::json!({"archive_sha256":expected_hash,"extracted":report}))
}

#[cfg(test)]
mod guard_tests {
    use super::*;
    use std::{
        io::Cursor,
        sync::atomic::{AtomicUsize, Ordering},
    };
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    struct Scratch(PathBuf);
    impl Scratch {
        fn new() -> Self {
            let p = std::env::temp_dir().join(format!(
                "mtg-catalog-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&p).unwrap();
            Self(p)
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn malformed_catalog_is_a_serializable_runtime_error() {
        let error = build_with(Path::new("catalogs/broken.json"), |_| Ok("{".into())).unwrap_err();
        assert!(matches!(error, CatalogError::InvalidManifest { .. }));
        let json = serde_json::to_string(&error).unwrap();
        assert_eq!(serde_json::from_str::<CatalogError>(&json).unwrap(), error);
    }
    #[test]
    fn embedded_inputs_exclude_expected_ir_and_policy_copies() {
        assert!(
            fixtures::FILES
                .iter()
                .all(|(name, _)| !name.starts_with("expected-")
                    && !name.ends_with(".sha256")
                    && !name.ends_with("policy.json"))
        );
        validate_embedded_record_groups().unwrap();
    }
    #[test]
    fn write_refuses_relabelling_a_record_with_another_accepted_header() {
        let scratch = Scratch::new();
        fs::create_dir(scratch.0.join("catalogs")).unwrap();
        let manifest_raw = embedded_file(Path::new("catalogs/base.json")).unwrap();
        let manifest: CatalogManifest = strict_read(&manifest_raw).unwrap();
        let mut provenance: FixtureProvenance =
            strict_read(&embedded_file(Path::new("provenance.json")).unwrap()).unwrap();
        provenance
            .records
            .iter_mut()
            .find(|r| r.file == "grizzly-bears.json")
            .unwrap()
            .archive = "archive-manifest-current.json".into();
        for file in [
            "accepted-archives.json",
            "archive-manifest.json",
            "archive-manifest-current.json",
        ] {
            fs::write(
                scratch.0.join(file),
                embedded_file(Path::new(file)).unwrap(),
            )
            .unwrap();
        }
        for file in &manifest.records {
            let file = relative(Path::new("catalogs"), file);
            fs::write(scratch.0.join(&file), embedded_file(&file).unwrap()).unwrap();
        }
        fs::write(
            scratch.0.join("provenance.json"),
            serde_json::to_vec(&provenance).unwrap(),
        )
        .unwrap();
        let path = scratch.0.join("catalogs/base.json");
        fs::write(&path, manifest_raw).unwrap();
        assert!(
            generate_catalog(&path, true)
                .unwrap_err()
                .to_string()
                .contains("labelled archive origin")
        );
        assert!(
            !scratch
                .0
                .join("catalogs")
                .join(&manifest.expected_ir)
                .exists()
        );
        assert!(
            !scratch
                .0
                .join("catalogs")
                .join(&manifest.expected_digest)
                .exists()
        );
    }
    #[test]
    fn one_catalog_can_validate_records_from_both_accepted_archives() {
        let p: FixtureProvenance =
            strict_read(&embedded_file(Path::new("provenance.json")).unwrap()).unwrap();
        let records = accepted_archives()
            .unwrap()
            .iter()
            .map(|a| {
                format!(
                    "../{}",
                    p.records
                        .iter()
                        .find(|r| r.archive == a.manifest_file)
                        .unwrap()
                        .file
                )
            })
            .collect::<Vec<_>>();
        let raw = serde_json::json!({"schema_version":CURRENT_IR_VERSION,"output_order":"canonical","provenance":"../provenance.json","records":records,"expected_ir":"unused.json","expected_digest":"unused.sha256"}).to_string();
        let (_, c) = build_with(Path::new("catalogs/mixed.json"), |path| {
            if path == Path::new("catalogs/mixed.json") {
                Ok(raw.clone())
            } else {
                embedded_file(path)
            }
        })
        .unwrap();
        assert_eq!(c.definitions.len(), records.len());
        assert_eq!(
            c.definitions
                .iter()
                .map(|d| &d.provenance.archive_manifest_sha256)
                .collect::<BTreeSet<_>>()
                .len(),
            records.len()
        );
    }
    #[test]
    fn manifest_rejects_duplicates_missing_witnesses_tampering_and_wrong_archive() {
        for change in [
            "duplicate",
            "missing",
            "record",
            "identity",
            "archive",
            "header",
            "version",
        ] {
            let result = build_with(Path::new("catalogs/base.json"), |p| {
                let raw = embedded_file(p)?;
                let mut v: serde_json::Value = strict_read(&raw)?;
                if p == Path::new("catalogs/base.json") {
                    if change == "duplicate" {
                        let first = v["records"][0].clone();
                        v["records"].as_array_mut().unwrap().push(first);
                    }
                    if change == "version" {
                        v["schema_version"] = 2.into();
                    }
                }
                if p == Path::new("provenance.json") {
                    let rows = v["records"].as_array_mut().unwrap();
                    if change == "missing" {
                        rows.retain(|r| r["file"] != "grizzly-bears.json");
                    }
                    if let Some(row) = rows.iter_mut().find(|r| r["file"] == "grizzly-bears.json") {
                        if change == "record" {
                            row["record_sha256"] = "0".repeat(64).into();
                        }
                        if change == "identity" {
                            row["oracle_id"] = "wrong".into();
                        }
                        if change == "archive" {
                            row["archive"] = "unknown.json".into();
                        }
                    }
                }
                if p == Path::new("archive-manifest.json") && change == "header" {
                    v["schemaVersion"] = 4.into();
                }
                // Preserve source/header bytes except where deliberately changed.
                if p == Path::new("catalogs/base.json")
                    || p == Path::new("provenance.json")
                    || (p == Path::new("archive-manifest.json") && change == "header")
                {
                    Ok(v.to_string())
                } else {
                    Ok(raw)
                }
            });
            assert!(result.is_err(), "{change}");
        }
    }
    fn extraction_case(
        selectors: &[String],
        corrupt: bool,
        override_raw: Option<Vec<u8>>,
    ) -> (Scratch, Result<serde_json::Value, String>, Vec<u8>) {
        use sha2::{Digest, Sha256};
        let scratch = Scratch::new();
        let header = fixtures::MANIFEST.as_bytes().to_vec();
        let raw = override_raw.unwrap_or_else(|| {
            embedded_file(Path::new("island.json"))
                .unwrap()
                .into_bytes()
        });
        let mut all = header.clone();
        all.extend(&raw);
        let mut accepted = accepted_manifest(fixtures::MANIFEST).unwrap().clone();
        accepted.file_sha256 = Some(format!("{:x}", Sha256::digest(&all)));
        fs::write(scratch.0.join(&accepted.manifest_file), &header).unwrap();
        fs::write(
            scratch.0.join("accepted-archives.json"),
            serde_json::to_vec(accepted_archives().unwrap()).unwrap(),
        )
        .unwrap();
        let provenance = FixtureProvenance {
            fixture_version: 1,
            rules_edition: RULES_EDITION.into(),
            rules_sha256: RULES_SHA256.into(),
            records: vec![],
        };
        let p = scratch.0.join("provenance.json");
        fs::write(&p, serde_json::to_vec(&provenance).unwrap()).unwrap();
        let mut body = raw.clone();
        if corrupt {
            body.extend(b"\n");
        }
        let result = extract_verified(Cursor::new(body), header, &accepted, &p, selectors);
        (scratch, result, raw)
    }
    #[test]
    fn extraction_preserves_raw_bytes_and_records_archive_and_line() {
        let island: serde_json::Value =
            strict_read(&embedded_file(Path::new("island.json")).unwrap()).unwrap();
        let id = island["oracleIdentity"].as_str().unwrap();
        let (scratch, result, raw) = extraction_case(&["Island".into(), id.into()], false, None);
        let report = result.unwrap();
        assert_eq!(report["extracted"][0]["archive_line"], 2);
        assert_eq!(fs::read(scratch.0.join(format!("{id}.json"))).unwrap(), raw);
        let p: FixtureProvenance =
            strict_read(&fs::read_to_string(scratch.0.join("provenance.json")).unwrap()).unwrap();
        assert_eq!(p.records[0].archive, "archive-manifest.json");
        assert_eq!(
            p.records[0].record_sha256,
            sha256(std::str::from_utf8(&raw).unwrap().trim_end().as_bytes())
        );
    }
    #[test]
    fn verification_checks_original_lines_bytes_pins_and_provenance() {
        for change in [
            "none",
            "terminator",
            "line",
            "missing_line",
            "past_end",
            "identity",
            "record_hash",
            "file_hash",
            "origin",
        ] {
            let (scratch, result, raw) = extraction_case(&["Island".into()], false, None);
            let report = result.unwrap();
            let file = report["extracted"][0]["file"].as_str().unwrap();
            let path = scratch.0.join("provenance.json");
            let mut p: FixtureProvenance = strict_read(&read_file(&path).unwrap()).unwrap();
            let mut accepted = accepted_manifest(fixtures::MANIFEST).unwrap().clone();
            let header = fixtures::MANIFEST.as_bytes();
            let mut all = header.to_vec();
            all.extend(&raw);
            accepted.file_sha256 = Some(sha256(&all));
            match change {
                "terminator" => {
                    fs::write(scratch.0.join(file), [raw.as_slice(), b"\n"].concat()).unwrap()
                }
                "line" => p.records[0].archive_line = Some(1),
                "missing_line" => p.records[0].archive_line = None,
                "past_end" => p.records[0].archive_line = Some(3),
                "identity" => p.records[0].oracle_id = "wrong".into(),
                "record_hash" => p.records[0].record_sha256 = "0".repeat(64),
                "file_hash" => accepted.file_sha256 = Some("0".repeat(64)),
                "origin" => p.records[0].archive = "archive-manifest-current.json".into(),
                _ => {}
            }
            fs::write(&path, serde_json::to_vec(&p).unwrap()).unwrap();
            let verified = verify_archive_with(Cursor::new(raw), header, &accepted, &path);
            if change == "none" {
                assert_eq!(verified.unwrap()["verified_records"], 1);
            } else {
                assert!(verified.is_err(), "{change}");
            }
        }
    }
    #[test]
    fn extraction_never_writes_for_bad_file_hash_missing_or_inexact_selector() {
        for (selectors, corrupt) in [
            (vec!["Island".into()], true),
            (vec!["island".into()], false),
            (vec!["Missing".into()], false),
        ] {
            let (scratch, result, _) = extraction_case(&selectors, corrupt, None);
            assert!(result.is_err());
            let p: FixtureProvenance =
                strict_read(&fs::read_to_string(scratch.0.join("provenance.json")).unwrap())
                    .unwrap();
            assert!(p.records.is_empty());
            assert_eq!(fs::read_dir(&scratch.0).unwrap().count(), 3);
        }
    }
    #[test]
    fn extraction_keeps_unparsed_records_without_inventing_an_ast_hash() {
        // Preserve the source/binding raw JSON spellings when changing only
        // these envelope fields; all are covered by the synthetic file hash.
        let raw = embedded_file(Path::new("island.json")).unwrap();
        let original: serde_json::Value = strict_read(&raw).unwrap();
        let changed = raw
            .replace(
                &format!(
                    "\"astSha256\":\"{}\"",
                    original["astSha256"].as_str().unwrap()
                ),
                "\"astSha256\":null",
            )
            .replace("\"status\":\"parsed\"", "\"status\":\"unparsed\"");
        let (scratch, result, bytes) =
            extraction_case(&["Island".into()], false, Some(changed.into_bytes()));
        let report = result.unwrap();
        let file = report["extracted"][0]["file"].as_str().unwrap();
        assert_eq!(fs::read(scratch.0.join(file)).unwrap(), bytes);
        let p: FixtureProvenance =
            strict_read(&fs::read_to_string(scratch.0.join("provenance.json")).unwrap()).unwrap();
        assert_eq!(p.records[0].ast_sha256, None);
        assert!(matches!(
            compile(fixtures::MANIFEST, std::str::from_utf8(&bytes).unwrap()),
            CompileResult::Unsupported { .. }
        ));
    }
    #[test]
    fn excluded_records_need_no_binding_or_faces_and_remain_nonexecutable() {
        // Preserve raw source spelling while modeling the documented excluded
        // envelope. The private stream boundary uses its own synthetic file pin.
        let mut record: std::collections::BTreeMap<String, Box<serde_json::value::RawValue>> =
            serde_json::from_str(&embedded_file(Path::new("island.json")).unwrap()).unwrap();
        for key in ["binding", "bindingSha256", "astSha256", "faces"] {
            record.remove(key);
        }
        record.insert(
            "status".into(),
            serde_json::value::RawValue::from_string("\"excluded\"".into()).unwrap(),
        );
        let mut raw = serde_json::to_vec(&record).unwrap();
        raw.push(b'\n');
        let (scratch, result, bytes) = extraction_case(&["Island".into()], false, Some(raw));
        let report = result.unwrap();
        let file = report["extracted"][0]["file"].as_str().unwrap();
        assert_eq!(fs::read(scratch.0.join(file)).unwrap(), bytes);
        let p: FixtureProvenance =
            strict_read(&fs::read_to_string(scratch.0.join("provenance.json")).unwrap()).unwrap();
        assert_eq!(p.records[0].ast_sha256, None);
        assert!(!matches!(
            compile(fixtures::MANIFEST, std::str::from_utf8(&bytes).unwrap()),
            CompileResult::Lowered { .. }
        ));
    }
    #[test]
    fn historical_archive_has_no_full_file_identity_and_cannot_be_extracted() {
        let scratch = Scratch::new();
        let path = scratch.0.join("historical.jsonl");
        fs::write(&path, fixtures::MANIFEST).unwrap();
        let result = extract_records(
            &path,
            &scratch.0.join("provenance.json"),
            &["Island".into()],
        );
        assert!(result.unwrap_err().contains("full archive file SHA-256"));
    }
}
