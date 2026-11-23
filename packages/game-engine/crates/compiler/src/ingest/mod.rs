//! Ingest: the exact archived record, source and per-face parse, and the
//! typed lowering mirror of its resolved AST blocks. Nothing here lowers.
use super::*;

mod mirror;
pub(crate) use mirror::*;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Record {
    pub(crate) record_type: String,
    pub(crate) source_index: u32,
    pub(crate) oracle_identity: String,
    pub(crate) source_sha256: String,
    pub(crate) source: Box<RawValue>,
    pub(crate) status: String,
    pub(crate) faces: Vec<Face>,
    pub(crate) ast_sha256: Option<String>,
    pub(crate) binding: Box<RawValue>,
    pub(crate) binding_sha256: String,
    /// Archive R4 (schema 4): the authoritative resolution. Absent from older
    /// archives, whose rows therefore reject.
    #[serde(default)]
    pub(crate) resolution: Option<Box<RawValue>>,
    #[serde(default)]
    pub(crate) resolution_sha256: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Face {
    pub(crate) source_face_index: u32,
    pub(crate) status: String,
    pub(crate) result: Box<RawValue>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ParseResult {
    pub(crate) name: String,
    pub(crate) ok: bool,
    pub(crate) lines: Vec<SourceLine>,
    pub(crate) abilities: Box<RawValue>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SourceLine {
    pub(crate) text: String,
    pub(crate) ok: bool,
    pub(crate) ability: Box<RawValue>,
}

// Unconsumed Scryfall metadata stays in the original RawValue. Additional
// metadata is allowed; every consumed field is typed and validated. Field
// meanings: https://scryfall.com/docs/api/cards
#[derive(Deserialize)]
pub(crate) struct Source {
    pub(crate) id: String,
    pub(crate) oracle_id: String,
    pub(crate) name: String,
    pub(crate) layout: String,
    pub(crate) oracle_text: String,
    pub(crate) mana_cost: String,
    pub(crate) type_line: String,
    pub(crate) colors: Vec<Color>,
    pub(crate) color_identity: Vec<Color>,
    pub(crate) power: Option<String>,
    pub(crate) toughness: Option<String>,
    pub(crate) card_faces: Option<Box<RawValue>>,
    pub(crate) loyalty: Option<Box<RawValue>>,
    pub(crate) defense: Option<Box<RawValue>>,
}

pub(crate) fn mana_cost(text: &str) -> Result<ManaCost, Failure> {
    let mut cost = ManaCost {
        generic: Quantity::zero(),
        colored: vec![],
    };
    let mut rest = text;
    while !rest.is_empty() {
        let token = rest
            .strip_prefix('{')
            .and_then(|s| s.split_once('}'))
            .ok_or_else(|| invalid("source.mana_cost", "malformed mana symbol"))?;
        match token.0 {
            "W" => cost.colored.push(Color::W),
            "U" => cost.colored.push(Color::U),
            "B" => cost.colored.push(Color::B),
            "R" => cost.colored.push(Color::R),
            "G" => cost.colored.push(Color::G),
            "C" => cost.colored.push(Color::C),
            digits if !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) => {
                let q: Quantity = digits.parse().map_err(|e| invalid("source.mana_cost", e))?;
                cost.generic.add(&q);
            }
            _ => {
                return Err(unsupported(
                    "source.mana_cost",
                    "variable/hybrid/Phyrexian/special mana is not supported",
                ));
            }
        }
        rest = token.1;
    }
    Ok(cost)
}

/// A single-face normal card record whose source and diagnostic-binding bytes
/// match their archived hashes, with a complete, name-matched parse whose AST
/// bytes match the archived AST hash (CR 201.5, 712). Composition reads only
/// these; resolution and block checks follow.
pub(crate) struct SingleFace {
    pub(crate) record: Record,
    pub(crate) shape: serde_json::Value,
    pub(crate) source: Source,
    pub(crate) result: ParseResult,
    pub(crate) ast_sha256: String,
}

pub(crate) fn read_single_face(record: &str) -> Result<SingleFace, Failure> {
    let r: Record = decode(record, "record")?;
    if r.record_type != "card" {
        return Err(invalid("recordType", "expected card record"));
    }
    if r.status != "parsed" {
        return Err(unsupported("status", "complete-card parsing is required"));
    }
    if sha256(r.source.get().as_bytes()) != r.source_sha256
        || sha256(r.binding.get().as_bytes()) != r.binding_sha256
    {
        return Err(invalid(
            "record",
            "source or diagnostic binding hash mismatch",
        ));
    }
    let shape: serde_json::Value = decode(r.source.get(), "source")?;
    if shape.get("card_faces").is_some_and(|v| !v.is_null()) {
        return Err(unsupported("source.layout", "multi-face layout"));
    }
    let source: Source = decode(r.source.get(), "source")?;
    if source.oracle_id != r.oracle_identity || source.id.is_empty() || source.name.is_empty() {
        return Err(invalid("source", "missing or mismatched card identity"));
    }
    if source.layout != "normal"
        || source.card_faces.is_some()
        || source.loyalty.is_some()
        || source.defense.is_some()
    {
        return Err(unsupported(
            "source.layout",
            "composition requires a single-face normal card",
        ));
    }
    if r.faces.len() != 1 || r.faces[0].source_face_index != 0 || r.faces[0].status != "attempted" {
        return Err(invalid("faces", "missing or duplicated source face"));
    }
    let result: ParseResult = decode(r.faces[0].result.get(), "faces[0].result")?;
    if !result.ok || result.name != source.name || result.lines.iter().any(|l| !l.ok) {
        return Err(invalid("faces[0].result", "incomplete or mismatched parse"));
    }
    let ast_sha256 = r
        .ast_sha256
        .clone()
        .ok_or_else(|| invalid("astSha256", "missing successful AST hash"))?;
    if sha256(format!("[{}]", result.abilities.get()).as_bytes()) != ast_sha256 {
        return Err(invalid("astSha256", "successful AST hash mismatch"));
    }
    Ok(SingleFace {
        record: r,
        shape,
        source,
        result,
        ast_sha256,
    })
}
