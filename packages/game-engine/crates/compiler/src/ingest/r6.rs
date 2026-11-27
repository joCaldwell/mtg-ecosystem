use super::{Face, Failure, RawValue, Record, decode, invalid, printed};
use mtg_contracts::{AcceptedArchive, ArchiveCut, R6Pins, Sha256Sink, sha256};
use mtg_oracle_ast::{printed::*, vocab::PRINTED_VOCABULARY_SHA256};
use serde::{Deserialize, Deserializer};
use serde_json::Value;

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum Status {
    Parsed,
    Unsupported,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct RecordR6 {
    record_type: String,
    source_index: u32,
    oracle_identity: String,
    source_sha256: String,
    source: Box<RawValue>,
    status: Status,
    faces: Vec<FaceR6>,
    #[serde(default, deserialize_with = "present")]
    ast_sha256: Option<String>,
    binding: Box<RawValue>,
    binding_sha256: String,
    resolution: Box<RawValue>,
    resolution_sha256: String,
    printed: Box<RawValue>,
    printed_sha256: String,
}

// A present hash must be a string. Unsupported cards omit the AST hash.
fn present<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<String>, D::Error> {
    String::deserialize(deserializer).map(Some)
}

fn raw_present<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Box<RawValue>>, D::Error> {
    Box::<RawValue>::deserialize(deserializer).map(Some)
}

#[derive(Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub(super) enum FaceStatus {
    Attempted,
    MissingOracleText,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct FaceR6 {
    pub status: FaceStatus,
    pub source_face_index: u32,
    #[serde(default, deserialize_with = "raw_present")]
    pub result: Option<Box<RawValue>>,
    #[serde(default, deserialize_with = "present")]
    pub name: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Excluded {
    record_type: String,
    source_index: u32,
    oracle_identity: String,
    source_sha256: String,
    source: Box<RawValue>,
    status: String,
    reason: String,
}

impl RecordR6 {
    pub(super) fn read(raw: &str) -> Result<Self, Failure> {
        decode(raw, "printed.P1")
    }

    pub(super) fn into_legacy(self) -> Result<Record, Failure> {
        let faces = self
            .faces
            .into_iter()
            .map(|face| match (face.status, face.result) {
                (FaceStatus::Attempted, Some(result)) => Ok(Face {
                    source_face_index: face.source_face_index,
                    status: "attempted".into(),
                    result,
                }),
                // Keep the R5 outcome for the missing-face legacy shape.
                _ => Err(invalid("record", "unknown field `name`")),
            })
            .collect::<Result<_, _>>()?;
        Ok(Record {
            record_type: self.record_type,
            source_index: self.source_index,
            oracle_identity: self.oracle_identity,
            source_sha256: self.source_sha256,
            source: self.source,
            status: match self.status {
                Status::Parsed => "parsed",
                Status::Unsupported => "unsupported",
            }
            .into(),
            faces,
            ast_sha256: self.ast_sha256,
            binding: self.binding,
            binding_sha256: self.binding_sha256,
            resolution: Some(self.resolution),
            resolution_sha256: Some(self.resolution_sha256),
        })
    }
}

fn failure(error: Failure) -> String {
    format!("{}: {}", error.path, error.reason)
}

pub(crate) struct ArchiveEvidence<'a>(Option<ArchiveR6<'a>>);

struct ArchiveR6<'a> {
    pins: &'a R6Pins,
    source_hash: Sha256Sink,
    evidence_hash: Sha256Sink,
    names: printed::NameCatalog,
    claims: printed::NameClaims,
    input_records: u32,
    bytes: u64,
    max_line: usize,
}

impl<'a> ArchiveEvidence<'a> {
    pub(crate) fn new(archive: &'a AcceptedArchive, header_bytes: usize) -> Result<Self, String> {
        if archive.cut != ArchiveCut::R6 {
            return Ok(Self(None));
        }
        let pins = archive.r6.as_ref().ok_or("R6 cut has no measured pins")?;
        if pins.printed_schema_sha256 != PRINTED_SCHEMA_SHA256
            || pins.printed_vocabulary_sha256 != PRINTED_VOCABULARY_SHA256
            || pins.printed_generator_sha256 != PRINTED_GENERATOR_SHA256
        {
            return Err("R6 printed decoder identity mismatch".into());
        }
        Ok(Self(Some(ArchiveR6 {
            pins,
            source_hash: Sha256Sink::new(),
            evidence_hash: Sha256Sink::new(),
            names: printed::NameCatalog::default(),
            claims: printed::NameClaims::default(),
            input_records: 0,
            bytes: header_bytes as u64,
            max_line: header_bytes,
        })))
    }

    pub(crate) fn record(
        &mut self,
        raw: &str,
        line_bytes: usize,
        line: usize,
    ) -> Result<(), String> {
        let Some(state) = &mut self.0 else {
            return Ok(());
        };
        state
            .record(raw, line_bytes)
            .map_err(|err| format!("archive line {line}: {}", failure(err)))
    }

    pub(crate) fn finish(self) -> Result<(), String> {
        let Some(state) = self.0 else {
            return Ok(());
        };
        if state.input_records != state.pins.input_records
            || state.bytes != state.pins.file_bytes
            || state.max_line != state.pins.max_line_bytes
            || state.source_hash.finalize_hex() != state.pins.canonical_records_sha256
            || state.evidence_hash.finalize_hex() != state.pins.printed_records_sha256
        {
            return Err("R6 archive population, size or ordered digest mismatch".into());
        }
        state.names.verify(&state.claims, state.pins)
    }

    pub(crate) fn verify_fixture(archive: &'a AcceptedArchive, raw: &str) -> Result<(), Failure> {
        if archive.cut != ArchiveCut::R6 {
            return Ok(());
        }
        Self::new(archive, 0).map_err(|e| invalid("manifest", e))?;
        let raw = raw.strip_suffix('\n').unwrap_or(raw);
        if !archive.record_sha256.contains(&sha256(raw.as_bytes())) {
            return Err(invalid(
                "record",
                "R6 fixture is not pinned to its archive origin",
            ));
        }
        verify_row(raw, &mut printed::NameClaims::default())?;
        // The reviewed raw-row pin carries the corpus-wide catalog check
        // performed at registration. A standalone fixture has no source corpus.
        Ok(())
    }

    pub(crate) fn verify_text(
        archive: &'a AcceptedArchive,
        text: &str,
    ) -> Result<(), (usize, String)> {
        if archive.cut != ArchiveCut::R6 {
            return Ok(());
        }
        let mut lines = text.split_inclusive('\n');
        let mut evidence =
            Self::new(archive, lines.next().map_or(0, str::len)).map_err(|error| (1, error))?;
        for (index, raw) in lines.enumerate() {
            let line = index + 2;
            let record = raw
                .strip_suffix('\n')
                .ok_or((line, "R6 row is missing LF".into()))?;
            evidence
                .record(record, raw.len(), line)
                .map_err(|error| (line, error))?;
        }
        evidence.finish().map_err(|error| (1, error))
    }
}

struct RowIdentity {
    index: u32,
    identity: String,
    source: Box<RawValue>,
    printed_sha256: String,
}

impl ArchiveR6<'_> {
    fn record(&mut self, raw: &str, line_bytes: usize) -> Result<(), Failure> {
        if line_bytes != raw.len() + 1 || !raw.starts_with('{') || !raw.ends_with('}') {
            return Err(invalid(
                "record",
                "R6 requires an exact JSON object followed by LF",
            ));
        }
        let row = verify_row(raw, &mut self.claims)?;
        if row.index != self.input_records || self.input_records >= self.pins.input_records {
            return Err(invalid("record", "R6 source index is out of sequence"));
        }
        self.input_records += 1;
        self.bytes += line_bytes as u64;
        self.max_line = self.max_line.max(line_bytes);
        self.source_hash.update(row.source.get().as_bytes());
        self.source_hash.update(b"\n");
        self.evidence_hash.update(
            format!("{}\t{}\t{}\n", row.index, row.identity, row.printed_sha256).as_bytes(),
        );
        self.names
            .add_source(&decode(row.source.get(), "source")?)
            .map_err(|e| invalid("source", e))?;
        Ok(())
    }
}

fn verify_row(raw: &str, claims: &mut printed::NameClaims) -> Result<RowIdentity, Failure> {
    // Choose the exclusion envelope within R6, never the wire schema.
    #[derive(Deserialize)]
    struct RowStatus {
        status: String,
    }
    let status: RowStatus =
        serde_json::from_str(raw).map_err(|e| invalid("printed.P1", e.to_string()))?;
    let (record_type, index, identity, source, source_hash, printed_sha256) =
        if status.status == "excluded" {
            let r: Excluded = decode(raw, "printed.P1")?;
            if r.status != "excluded" || r.reason.is_empty() {
                return Err(invalid("printed.P1", "invalid exclusion"));
            }
            (
                r.record_type,
                r.source_index,
                r.oracle_identity,
                r.source,
                r.source_sha256,
                "excluded".into(),
            )
        } else {
            let r = RecordR6::read(raw)?;
            let source: Value = decode(r.source.get(), "printed.P2/source")?;
            if sha256(r.source.get().as_bytes()) != r.source_sha256 {
                return Err(invalid("printed.P2/source", "source SHA-256 mismatch"));
            }
            if sha256(r.printed.get().as_bytes()) != r.printed_sha256 {
                return Err(invalid("printed.P1", "printed SHA-256 mismatch"));
            }
            if sha256(r.binding.get().as_bytes()) != r.binding_sha256
                || sha256(r.resolution.get().as_bytes()) != r.resolution_sha256
            {
                return Err(invalid(
                    "record",
                    "legacy binding or resolution SHA-256 mismatch",
                ));
            }
            let evidence = PrintedCard::decode(r.printed.get())
                .map_err(|e| invalid("printed.P1", e.to_string()))?;
            let parsed = printed::verify(&source, &r.faces, &evidence, claims)?;
            if parsed != matches!(r.status, Status::Parsed) || parsed != r.ast_sha256.is_some() {
                return Err(invalid(
                    "printed.P1",
                    "AST hash presence disagrees with card status",
                ));
            }
            (
                r.record_type,
                r.source_index,
                r.oracle_identity,
                r.source,
                r.source_sha256,
                r.printed_sha256,
            )
        };
    let decoded: Value = decode(source.get(), "source")?;
    if record_type != "card"
        || !mtg_contracts::bulk_catalog::is_oracle_id(&identity)
        || decoded["oracle_id"] != identity
        || sha256(source.get().as_bytes()) != source_hash
    {
        return Err(invalid("record", "invalid R6 source identity or hash"));
    }
    Ok(RowIdentity {
        index,
        identity,
        source,
        printed_sha256,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const ID: &str = "12345678-1234-1234-1234-123456789abc";

    fn print_row(original: &str, normalized: &str) -> Value {
        let ability = json!({
            "kind": "spell", "effects": [{
                "effect": "draw", "count": {"amount": "fixed", "value": 3},
                "target": {"selectionId": "line:0:1"}
            }]
        });
        let tokens: Vec<_> = normalized.split_ascii_whitespace().enumerate().scan(0, |start, (index, word)| {
            let n = [*start, *start + word.len()];
            *start = n[1] + 1;
            let o = if index == 0 { [0, 7] } else { n.map(|x| x + 6) };
            Some(json!({"t": if index == 0 { "selfref" } else { "word" }, "n": n, "o": o, "grammarIndex": index}))
        }).collect();
        let source = json!({"oracle_id": ID, "name": "Example", "layout": "normal",
            "oracle_text": original, "type_line": "Sorcery"});
        let empty = sha256(b"{}");
        let blocks = json!([{"status":"read","lines":[0,1],"n":[0,normalized.len()],
                    "nodes":[{"path":"","basis":"hull","span":[[0,0,3]]},
                        {"path":"/effects/0","basis":"recorded","span":[[0,0,3]]},
                        {"path":"/effects/0/count","basis":"recorded","span":[[0,2,3]]},
                        {"path":"/effects/0/target","basis":"recorded","span":[[0,1,2]]}],
                    "facts":[{"fact":"target","at":"/effects/0/target","tok":[0,1,2],
                        "field":"/effects/0/target/selectionId","targetIndex":0,"scope":"","selectionId":"line:0:1"},
                        {"fact":"quantity","at":"/effects/0/count","tok":[0,2,3],
                        "field":"/effects/0/count/value","form":"word","value":3}]}]);
        json!({
            "recordType": "card", "sourceIndex": 0, "oracleIdentity": ID,
            "sourceSha256": sha256(source.to_string().as_bytes()), "source": source,
            "status": "parsed", "faces": [{"status":"attempted", "sourceFaceIndex": 0,
                "result":{"name":"Example", "ok":true, "lines":[{"text":normalized, "ok":true, "ability":ability}],
                "abilities":[ability]}}],
            "astSha256": sha256(json!([ability]).to_string().as_bytes()),
            "binding": {}, "bindingSha256": empty, "resolution": {}, "resolutionSha256": empty,
            "printed": {"artifactKind":"oracle-printed-evidence", "schemaVersion":1, "faces":[{
                "sourceFaceIndex": 0, "status": "read",
                "original":{"bytes":original.len(),"sha256":sha256(original.as_bytes())},
                "normalized":{"bytes":normalized.len(),"sha256":sha256(normalized.as_bytes())},
                "edits":[{"edit":"self-name","form":"full","o":[0,7],"n":[0,1]}],
                "lines":[{"status":"lexed","sourceId":"line:0","n":[0,normalized.len()],"tokens":tokens}],
                "removedReminders":[],
                "blocks":blocks}]}

        })
    }

    fn rehash(row: &mut Value) {
        row["printedSha256"] = sha256(row["printed"].to_string().as_bytes()).into();
    }

    #[test]
    fn every_printed_integrity_check_rejects_its_own_mutation() {
        let mut baseline = print_row("Example target three", "~ target three");
        rehash(&mut baseline);
        verify_row(&baseline.to_string(), &mut printed::NameClaims::default()).unwrap();
        let mutations = [
            ("P1", "/printedSha256".to_owned(), json!("stale")),
            ("P2", "/printed/faces/0/original/bytes".to_owned(), json!(2)),
            (
                "P3",
                "/printed/faces/0/edits/0/form".to_owned(),
                json!("first-word"),
            ),
            (
                "P4",
                "/printed/faces/0/lines/0/tokens/1/o".to_owned(),
                json!([7, 13]),
            ),
            (
                "P5",
                "/printed/faces/0/lines/0/tokens/1/grammarIndex".to_owned(),
                json!(2),
            ),
            (
                "P6",
                "/printed/faces/0/blocks/0/nodes/0/span".to_owned(),
                json!([[0, 0, 2]]),
            ),
            (
                "P7",
                "/printed/faces/0/blocks/0/facts/1/at".to_owned(),
                json!("/effects/0"),
            ),
            (
                "P8",
                "/printed/faces/0/blocks/0/facts/1/value".to_owned(),
                json!(2),
            ),
            (
                "P9",
                "/printed/faces/0/blocks/0/facts/0/targetIndex".to_owned(),
                json!(1),
            ),
            (
                "P10",
                "/printed/faces/0/blocks/0/facts/1/field".to_owned(),
                json!("/effects/0/target"),
            ),
        ];
        for (check, path, value) in mutations {
            let mut row = baseline.clone();
            *row.pointer_mut(&path).unwrap() = value;
            if check != "P1" {
                rehash(&mut row);
            }
            let err = verify_row(&row.to_string(), &mut printed::NameClaims::default())
                .err()
                .unwrap();
            assert_eq!(
                err.path.rsplit(['.', '/']).next(),
                Some(check),
                "{path}: {err:?}"
            );
        }

        // A test that bypasses the whole-file pin still re-reads archived O.
        let mut row = baseline;
        row["source"]["oracle_text"] = "Example target twice".into();
        row["sourceSha256"] = sha256(row["source"].to_string().as_bytes()).into();
        let err = verify_row(&row.to_string(), &mut printed::NameClaims::default())
            .err()
            .unwrap();
        assert!(err.path.ends_with("/P2"), "{err:?}");
    }

    #[test]
    fn envelopes_are_cut_specific_and_exclusion_has_no_sidecar() {
        let mut row = print_row("Example target three", "~ target three");
        rehash(&mut row);
        assert!(decode::<Record>(&row.to_string(), "record").is_err());
        let map = row.as_object_mut().unwrap();
        map.remove("printed");
        map.remove("printedSha256");
        assert!(decode::<Record>(&row.to_string(), "record").is_ok());
        assert!(RecordR6::read(&row.to_string()).is_err());
        let mut excluded = json!({
            "recordType": "card", "status":"excluded","reason":"layout:token",
            "sourceIndex":0,"oracleIdentity":ID,"source":row["source"],"sourceSha256":row["sourceSha256"],
        });
        let verify =
            |value: &Value| verify_row(&value.to_string(), &mut printed::NameClaims::default());
        assert!(verify(&excluded).is_ok());
        excluded["printed"] = Value::Null;
        assert_eq!(verify(&excluded).err().unwrap().path, "printed.P1");
    }

    #[test]
    #[ignore = "opt-in unregistered sidecar diagnostic"]
    fn verify_candidate_rows() {
        use std::collections::BTreeMap;
        use std::io::BufRead;
        let path = std::env::var_os("MTG_R6_VERIFIER_ROWS").expect("set MTG_R6_VERIFIER_ROWS");
        let reader = std::io::BufReader::new(std::fs::File::open(path).unwrap());
        let mut claims = printed::NameClaims::default();
        let mut failures = BTreeMap::<String, usize>::new();
        let mut count = 0;
        for (index, line) in reader.lines().enumerate() {
            let line = line.unwrap();
            if index == 0 && line.starts_with("{\"recordType\":\"manifest\"") {
                continue;
            }
            count += 1;
            if let Err(err) = verify_row(&line, &mut claims) {
                let check = err.path.rsplit(['.', '/']).next().unwrap();
                let n = failures.entry(check.to_owned()).or_default();
                *n += 1;
                if *n <= 30 {
                    let record: Value = serde_json::from_str(&line).unwrap();
                    eprintln!(
                        "{} {}: {}",
                        record["sourceIndex"],
                        record["source"]["name"],
                        failure(err)
                    );
                }
            }
        }
        assert!(
            count != 0 && failures.is_empty(),
            "rows={count}, failures={failures:?}"
        );
    }
}
