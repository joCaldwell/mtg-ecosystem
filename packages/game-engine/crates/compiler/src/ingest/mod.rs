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
    /// Archive R4 (schema 4): the authoritative resolution. Absent from the
    /// schema-3 fixture archives the whole-card families still read.
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

pub(crate) fn read_abilities(raw: &str) -> Result<Vec<Ability>, Failure> {
    // Keep the duplicate-key pass before serde's internally tagged mirrors.
    // serde_path_to_error reports context without accepting unknown fields.
    let nodes: Vec<Box<RawValue>> = decode(raw, "abilities")?;
    nodes
        .iter()
        .enumerate()
        .map(|(i, node)| {
            serde_json::from_str::<Ability>(node.get())
                .map_err(|_| diagnostics::ability(node.get(), &format!("abilities[{i}]")))
        })
        .collect()
}
