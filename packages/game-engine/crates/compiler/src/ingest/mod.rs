//! Ingest: the exact archived record, source and per-face parse, and the
//! typed lowering mirror of its resolved AST blocks. Nothing here lowers.
use super::*;

mod mirror;
mod printed;
mod r6;
pub(crate) use mirror::*;
pub(crate) use r6::ArchiveEvidence;

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
    colors: Option<Vec<Color>>,
    #[serde(default)]
    keywords: Vec<String>,
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
    /// Split and adventurer parts derive colors from their mana costs.
    pub(crate) colors: Option<Vec<Color>>,
    pub(crate) power: Option<String>,
    pub(crate) toughness: Option<String>,
    pub(crate) loyalty: Option<String>,
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

/// The mana symbols of one printed activation cost (CR 107.3, 107.4e-f,
/// 602.1a): its fixed symbols, its ordered two-color hybrid and
/// single-color Phyrexian symbols, and whether it begins with one `{X}`.
pub(crate) struct ActivationMana {
    pub(crate) base: ManaCost,
    pub(crate) hybrid: Vec<ColorHybridSymbol>,
    pub(crate) x: bool,
}

/// CR 107.3, 107.4e-f, 602.2b: each parser symbol is one complete mana
/// symbol. One leading X and two-color hybrid or single-color Phyrexian
/// symbols are announced; any other special symbol rejects.
pub(crate) fn activation_mana(symbols: &[String], field: &str) -> Result<ActivationMana, Failure> {
    if symbols.is_empty()
        || symbols
            .iter()
            .any(|s| s.is_empty() || s.contains(['{', '}']))
    {
        return Err(unsupported(field, "invalid mana-cost symbol"));
    }
    let x = symbols[0] == "X";
    let rest = &symbols[usize::from(x)..];
    if rest.iter().any(|s| s == "X") {
        return Err(unsupported("source.mana_cost", "repeated or trailing X"));
    }
    let text: String = rest.iter().map(|s| format!("{{{s}}}")).collect();
    let (base, hybrid) = if text.contains('/') {
        color_hybrid_cost(&text).ok_or_else(|| {
            unsupported(
                "source.mana_cost",
                "variable/hybrid/Phyrexian/special mana is not supported",
            )
        })?
    } else {
        (mana_cost(&text)?, vec![])
    };
    Ok(ActivationMana { base, hybrid, x })
}

pub(crate) struct CardSource {
    pub(crate) layout: FaceLayout,
    pub(crate) record: Record,
    pub(crate) shape: serde_json::Value,
    pub(crate) source: Source,
    pub(crate) faces: Vec<(SourceFace, ParseResult)>,
    pub(crate) ast_sha256: String,
}

impl CardSource {
    /// The card's printed colors (Scryfall's top-level `colors`).
    pub(crate) fn printed_colors(&self) -> Option<&[Color]> {
        self.source.colors.as_deref()
    }
}

/// CR 712.3, 712.8, 715.2: each face of a double-faced card, and each part
/// of an adventurer, is an independent printed set of characteristics.
/// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
pub(crate) fn read_card(archive: &AcceptedArchive, record: &str) -> Result<CardSource, Failure> {
    let r: Record = match archive.cut {
        ArchiveCut::R4 | ArchiveCut::R5 => decode(record, "record")?,
        ArchiveCut::R6 => r6::RecordR6::read(record)?.into_legacy()?,
    };
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
    let layout = match source.layout.as_str() {
        "normal" | "saga" => FaceLayout::Single,
        "modal_dfc" => FaceLayout::ModalDoubleFaced,
        "transform" => FaceLayout::Transforming,
        "adventure" => FaceLayout::Adventurer,
        "split" => FaceLayout::Split,
        layout => return Err(unsupported("source.layout", format!("layout:{layout}"))),
    };
    let printed: Vec<SourceFace> = match (layout, &source.card_faces) {
        _ if (layout != FaceLayout::Single && source.loyalty.is_some())
            || source.defense.is_some() =>
        {
            return Err(unsupported(
                "source.layout",
                "unsupported normal or two-part face shell",
            ));
        }
        (FaceLayout::Single, None) => vec![decode(r.source.get(), "source")?],
        (FaceLayout::Single, Some(_)) | (_, None) => {
            return Err(unsupported(
                "source.layout",
                "unsupported normal or two-part face shell",
            ));
        }
        (_, Some(faces)) => {
            if layout == FaceLayout::Split && r.faces.len() != 2 {
                return Err(unsupported(
                    "source.card_faces",
                    "expected two split halves",
                ));
            }
            let faces: [SourceFace; 2] = decode(faces.get(), "source.card_faces")?;
            if layout == FaceLayout::Split {
                // CR 709.5, 702.102, 702.127: each shell needs its own rules.
                if faces
                    .iter()
                    .any(|f| f.type_line.split_whitespace().any(|t| t == "Room"))
                {
                    return Err(unsupported("source.type_line", "subtype:Room"));
                }
                for (printed, reason) in
                    [("Fuse", "keyword:fuse"), ("Aftermath", "keyword:aftermath")]
                {
                    if source.keywords.iter().any(|k| k == printed) {
                        return Err(unsupported("source.keywords", reason));
                    }
                }
            }
            if source.name != format!("{} // {}", faces[0].name, faces[1].name) {
                return Err(invalid(
                    "source.name",
                    "card name does not match its printed parts",
                ));
            }
            // CR 709.4b, 715.2, 202.2: split and adventurer parts have no color indicator,
            // and each part's colors follow its own mana cost; double-faced
            // faces print their colors (CR 712.8).
            let colors_from_cost = matches!(layout, FaceLayout::Adventurer | FaceLayout::Split);
            if faces.iter().any(|face| {
                face.colors.is_some() == colors_from_cost
                    || (colors_from_cost && face.color_indicator.is_some())
            }) || (colors_from_cost && source.colors.is_none())
            {
                return Err(unsupported(
                    "source.face",
                    "unsupported part colors or color indicator",
                ));
            }
            faces.into()
        }
    };
    // CR 714.1a, 702.155: these variants require independent mechanics.
    for face in &printed {
        let (types, subtypes) = face.type_line.split_once(" — ").unwrap_or(("", ""));
        if !subtypes.split_whitespace().any(|s| s == "Saga") {
            continue;
        }
        if layout != FaceLayout::Single {
            return Err(unsupported("source.layout", "layout:multiface_saga"));
        }
        if types.split_whitespace().any(|t| t == "Creature") {
            return Err(unsupported("source.type_line", "subtype:saga_creature"));
        }
        if face.oracle_text.lines().any(|line| {
            line.eq_ignore_ascii_case("Read ahead")
                || line.to_ascii_lowercase().starts_with("read ahead (")
        }) {
            return Err(unsupported("source.keywords", "keyword:read_ahead"));
        }
    }
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
            if (layout != FaceLayout::Single && source.loyalty.is_some())
                || source.defense.is_some()
                || source.color_indicator.as_ref().is_some_and(|colors| {
                    colors.is_empty()
                        || colors
                            .iter()
                            .any(|c| !source.colors.as_ref().is_some_and(|all| all.contains(c)))
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
        layout,
        record: r,
        shape,
        source,
        faces,
        ast_sha256,
    })
}
