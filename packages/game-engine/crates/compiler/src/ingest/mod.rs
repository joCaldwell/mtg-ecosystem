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
    oracle_id: String,
    name: String,
    layout: String,
    pub(crate) color_identity: Vec<Color>,
    card_faces: Option<Box<RawValue>>,
    loyalty: Option<Box<RawValue>>,
    defense: Option<Box<RawValue>>,
}

#[derive(Deserialize)]
pub(crate) struct SourceFace {
    pub(crate) name: String,
    pub(crate) oracle_text: String,
    pub(crate) mana_cost: String,
    pub(crate) type_line: String,
    pub(crate) colors: Vec<Color>,
    pub(crate) power: Option<String>,
    pub(crate) toughness: Option<String>,
    loyalty: Option<Box<RawValue>>,
    defense: Option<Box<RawValue>>,
    color_indicator: Option<Vec<Color>>,
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

pub(crate) struct CardSource {
    pub(crate) record: Record,
    pub(crate) shape: serde_json::Value,
    pub(crate) source: Source,
    pub(crate) faces: Vec<(SourceFace, ParseResult)>,
    pub(crate) ast_sha256: String,
}

/// CR 712.3, 712.8: modal faces are independent printed sets of characteristics.
/// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
pub(crate) fn read_card(record: &str) -> Result<CardSource, Failure> {
    let r: Record = decode(record, "record")?;
    if r.record_type != "card" {
        return Err(invalid("recordType", "expected card record"));
    }
    if sha256(r.source.get().as_bytes()) != r.source_sha256
        || sha256(r.binding.get().as_bytes()) != r.binding_sha256
    {
        return Err(invalid(
            "record",
            "source or diagnostic binding hash mismatch",
        ));
    }
    let source: Source = decode(r.source.get(), "source")?;
    if source.oracle_id != r.oracle_identity || source.id.is_empty() || source.name.is_empty() {
        return Err(invalid("source", "missing or mismatched card identity"));
    }
    let printed: Vec<SourceFace> = match (source.layout.as_str(), &source.card_faces) {
        ("normal", None) if source.loyalty.is_none() && source.defense.is_none() => {
            vec![decode(r.source.get(), "source")?]
        }
        ("modal_dfc", Some(faces)) if source.loyalty.is_none() && source.defense.is_none() => {
            let faces: [SourceFace; 2] = decode(faces.get(), "source.card_faces")?;
            if source.name != format!("{} // {}", faces[0].name, faces[1].name) {
                return Err(invalid(
                    "source.name",
                    "card name does not match its modal faces",
                ));
            }
            faces.into()
        }
        ("normal" | "modal_dfc", _) => {
            return Err(unsupported(
                "source.layout",
                "unsupported normal or modal face shell",
            ));
        }
        (layout, _) => {
            return Err(unsupported("source.layout", format!("layout:{layout}")));
        }
    };
    if r.status != "parsed" {
        return Err(unsupported("status", "complete-card parsing is required"));
    }
    if printed.len() != r.faces.len() {
        return Err(invalid("faces", "missing or duplicated source face"));
    }
    let ast_sha256 = r
        .ast_sha256
        .clone()
        .ok_or_else(|| invalid("astSha256", "missing successful AST hash"))?;
    let faces = printed
        .into_iter()
        .zip(&r.faces)
        .enumerate()
        .map(|(index, (source, face))| {
            if face.source_face_index as usize != index || face.status != "attempted" {
                return Err(invalid("faces", "missing or duplicated source face"));
            }
            if source.loyalty.is_some()
                || source.defense.is_some()
                || source.color_indicator.as_ref().is_some_and(|colors| {
                    colors.is_empty() || colors.iter().any(|c| !source.colors.contains(c))
                })
            {
                return Err(unsupported(
                    "source.face",
                    "loyalty, defense or inconsistent color indicator",
                ));
            }
            let result: ParseResult = decode(face.result.get(), "faces.result")?;
            if !result.ok || result.name != source.name || result.lines.iter().any(|line| !line.ok)
            {
                return Err(invalid("faces.result", "incomplete or mismatched parse"));
            }
            Ok((source, result))
        })
        .collect::<Result<Vec<_>, Failure>>()?;
    let abilities = faces
        .iter()
        .map(|(_, result)| result.abilities.get())
        .collect::<Vec<_>>();
    if sha256(format!("[{}]", abilities.join(",")).as_bytes()) != ast_sha256 {
        return Err(invalid("astSha256", "successful AST hash mismatch"));
    }
    let shape = decode(r.source.get(), "source")?;
    Ok(CardSource {
        record: r,
        shape,
        source,
        faces,
        ast_sha256,
    })
}
