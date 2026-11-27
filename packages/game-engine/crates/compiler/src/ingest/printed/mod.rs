//! Common R6 integrity checks. The legacy lowerer never receives this data.
use super::r6::{FaceR6, FaceStatus};
use super::{Failure, RawValue, decode, invalid, sha256};
use mtg_oracle_ast::printed::*;
use serde::Deserialize;
use serde_json::Value;

// No production family opts into this API until its D3 migration.
#[allow(
    dead_code,
    reason = "Stage 2 family migrations opt in after the inert R6 cut"
)]
pub(crate) mod evidence;
mod facts;
mod nodes;
mod text;
mod tokens;
pub(super) use text::{NameCatalog, NameClaims};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyResult {
    name: String,
    ok: bool,
    lines: Vec<LegacyLine>,
    abilities: Option<Box<RawValue>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyLine {
    text: String,
    ok: bool,
    ability: Option<Value>,
    error: Option<String>,
}

pub(super) fn verify(
    source: &Value,
    faces: &[FaceR6],
    printed: &PrintedCard,
    claims: &mut NameClaims,
) -> Result<Option<String>, Failure> {
    let source_faces = match &source["card_faces"] {
        Value::Null => std::slice::from_ref(source),
        Value::Array(faces) if !faces.is_empty() => faces,
        _ => return Err(invalid("printed.P1", "invalid source face inventory")),
    };
    if source_faces.len() != faces.len() || faces.len() != printed.faces.len() {
        return Err(invalid(
            "printed.P1",
            "source, legacy and printed face inventories differ",
        ));
    }
    let mut all_parsed = true;
    let mut raw_abilities = Vec::new();
    for (index, ((source_face, legacy), printed)) in source_faces
        .iter()
        .zip(faces)
        .zip(&printed.faces)
        .enumerate()
    {
        let path = format!("printed/faces/{index}");
        let reject = |check: &str, detail: &str| invalid(&format!("{path}/{check}"), detail);
        let (printed_index, field) = match printed {
            PrintedFace::Read(value) => (value.source_face_index, None),
            PrintedFace::MissingOracleText(value) => {
                (value.source_face_index, Some(value.field.as_str()))
            }
            PrintedFace::Unmappable(value) => (value.source_face_index, None),
        };
        if legacy.source_face_index as usize != index || printed_index as usize != index {
            return Err(reject("P1", "face index mismatch"));
        }
        let name = source_face["name"]
            .as_str()
            .ok_or_else(|| reject("P2", "missing source face name"))?;
        let value = source_face.get("oracle_text");
        let (original, result) = match (
            value,
            legacy.status == FaceStatus::Attempted,
            &legacy.result,
            &legacy.name,
        ) {
            (Some(Value::String(original)), true, Some(result), None) if field.is_none() => {
                (original, result)
            }
            (None | Some(Value::Null), false, None, Some(legacy_name)) if legacy_name == name => {
                let expected = if value.is_none() { "absent" } else { "null" };
                if field != Some(expected) {
                    return Err(reject("P2", "missing-text presence mismatch"));
                }
                all_parsed = false;
                continue;
            }
            _ => return Err(reject("P2", "source oracle_text and face state disagree")),
        };
        let legacy: LegacyResult = decode(result.get(), &format!("{path}/P2"))?;
        if legacy.name != name {
            return Err(reject("P2", "legacy face name mismatch"));
        }
        all_parsed &= legacy.ok;
        match (legacy.ok, &legacy.abilities) {
            (true, Some(raw)) => {
                let abilities: Vec<Value> = decode(raw.get(), &format!("{path}/P6"))?;
                if abilities.len() != legacy.lines.len()
                    || !abilities
                        .iter()
                        .zip(&legacy.lines)
                        .all(|(ability, line)| line.ok && line.ability.as_ref() == Some(ability))
                {
                    return Err(reject("P6", "legacy ability array differs from its blocks"));
                }
                raw_abilities.push(raw.get().to_owned());
            }
            (false, None) if legacy.lines.iter().any(|line| !line.ok) => {}
            _ => {
                return Err(reject(
                    "P6",
                    "legacy ability array and parse status disagree",
                ));
            }
        }
        let type_line = source_face["type_line"]
            .as_str()
            .or_else(|| source["type_line"].as_str())
            .unwrap_or("");
        match printed {
            PrintedFace::Read(value) => {
                verify_face(original, &legacy.lines, value, name, type_line, claims)
                    .map_err(|error| invalid(&format!("{path}/{}", error.path), error.reason))?
            }
            PrintedFace::Unmappable(value)
                if (original.contains('~')
                    && value.reason == PrintedFaceUnmappableReason::RawTilde)
                    || (!original.contains('~')
                        && original.contains('\0')
                        && value.reason == PrintedFaceUnmappableReason::RawNul) => {}
            PrintedFace::Unmappable(_) => return Err(reject("P2", "unproved unmappable face")),
            PrintedFace::MissingOracleText(_) => unreachable!(),
        }
    }
    Ok(all_parsed.then(|| sha256(format!("[{}]", raw_abilities.join(",")).as_bytes())))
}

fn verify_face(
    original: &str,
    legacy: &[LegacyLine],
    face: &PrintedFaceRead,
    name: &str,
    type_line: &str,
    claims: &mut NameClaims,
) -> Result<(), Failure> {
    let normalized = legacy
        .iter()
        .map(|line| line.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    if original.len() != face.original.bytes as usize
        || sha256(original.as_bytes()) != face.original.sha256
        || normalized.len() != face.normalized.bytes as usize
        || sha256(normalized.as_bytes()) != face.normalized.sha256
    {
        return Err(invalid("P2", "original or normalized text digest mismatch"));
    }
    text::verify(
        original,
        &normalized,
        &face.edits,
        &face.removed_reminders,
        name,
        type_line,
        claims,
    )?;
    let lines = tokens::verify(original, &normalized, &face.lines, &face.edits, claims)?;
    if face.blocks.len() != legacy.len() {
        return Err(invalid("P6", "printed and legacy blocks differ in count"));
    }
    let mut first = 0;
    let mut n_start = 0;
    for (index, (block, legacy)) in face.blocks.iter().zip(legacy).enumerate() {
        let reject = |check, reason| invalid(check, format!("block {index}: {reason}"));
        let count = if legacy.text.is_empty() {
            0
        } else {
            legacy.text.split('\n').count()
        };
        let end = first + count;
        let partition = [first as u32, end as u32];
        let span = if count == 0 {
            [normalized.len() as u32; 2]
        } else {
            [n_start as u32, (n_start + legacy.text.len()) as u32]
        };
        match block {
            PrintedBlock::Read(block)
                if legacy.ok
                    && legacy.error.is_none()
                    && block.lines.0 == partition
                    && block.n.0 == span =>
            {
                let ast = legacy
                    .ability
                    .as_ref()
                    .ok_or_else(|| reject("P6", "missing legacy block"))?;
                let nodes = nodes::BlockNodes::verify(ast, &block.nodes, first..end, &lines)?;
                facts::verify(
                    &block.facts,
                    &nodes,
                    &lines,
                    first..end,
                    &face.edits,
                    &face.removed_reminders,
                    original,
                )?;
            }
            PrintedBlock::Failed(block)
                if !legacy.ok
                    && legacy.ability.is_none()
                    && legacy.error.as_deref() == Some(&block.error)
                    && block.lines.0 == partition
                    && block.n.0 == span =>
            {
                tokens::verify_failure(block, &lines)?;
                facts::verify(
                    &block.facts,
                    &nodes::BlockNodes::default(),
                    &lines,
                    first..end,
                    &face.edits,
                    &face.removed_reminders,
                    original,
                )?;
            }
            _ => return Err(reject("P6", "block range, status or legacy parse mismatch")),
        }
        first = end;
        n_start += legacy.text.len() + 1;
    }
    if first != lines.len() {
        return Err(invalid("P6", "blocks do not partition normalized lines"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn typescript_golden_normalizations_replay_in_rust() {
        let vectors: Vec<Value> = serde_json::from_str(include_str!(
            "../../../../../tests/fixtures/printed-text-golden.json"
        ))
        .unwrap();
        for vector in vectors {
            let Some(original) = vector["text"].as_str() else {
                continue;
            };
            let Some(normalized) = vector["normalized"].as_str() else {
                continue;
            };
            let Some(edits) = vector["edits"].as_array() else {
                assert!(vector["reason"].is_string());
                continue;
            };
            let removed: Vec<_> =
                edits
                    .iter()
                    .enumerate()
                    .flat_map(|(index, edit)| {
                        edit["reminder"].as_array().into_iter().flatten().map(
                            move |o| json!({"fact":"removed-reminder", "editIndex":index, "o":o}),
                        )
                    })
                    .collect();
            let wire = json!({
                "sourceFaceIndex":0,"status":"read","original":{"bytes":0,"sha256":""},
                "normalized":{"bytes":0,"sha256":""}, "edits":edits,"removedReminders":removed,
                "blocks":[],"lines":[],
            });
            let PrintedFace::Read(face) = PrintedFace::decode(&wire.to_string()).unwrap() else {
                unreachable!()
            };
            let mut claims = NameClaims::default();
            let result = text::verify(
                original,
                normalized,
                &face.edits,
                &face.removed_reminders,
                vector["name"].as_str().unwrap_or(""),
                vector["typeLine"].as_str().unwrap_or(""),
                &mut claims,
            );
            assert!(result.is_ok(), "{}: {result:?}", vector["id"]);
        }
    }
}
