//! Ordered registry of whole-card Rust source authorities.
//!
//! Composition asks the registry before it decodes any binder verdict,
//! projection or segment. Each family claims a card only from its complete
//! typed AST, with every field destructured, so a cross-family or
//! extra-field card stays entirely on its existing authority. Two families
//! claiming one card is a registration fault, never a precedence rule.
//! A claimed card never falls back to the archive's resolution graph,
//! including on a proof failure.
//!
//! Families and their supported and rejected shapes:
//! [`source_action`](super::source_action).
use super::*;
use crate::ingest::evidence::CardEvidence;
use serde_json::{Value, json};
use source_action::SourceAction;

/// The printed occurrence a family claimed. A claim names its own face,
/// block and step from the AST it matched.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Site {
    pub face: usize,
    pub block: usize,
    /// The step's AST pointer within its block.
    pub step: String,
}

type Select = fn(&[ast::Ability]) -> Option<SourceAction>;

/// Registration order is the report order; it never resolves a conflict.
const FAMILIES: &[(&str, Select)] = &[
    ("draw", source_action::draw),
    ("mill", source_action::mill),
    ("gain-life", source_action::gain_life),
    ("lose-life", source_action::lose_life),
    ("discard", source_action::discard),
];

pub(super) struct Claim {
    pub action: SourceAction,
    pub types: type_line::ParsedTypes,
}

/// The single family that claims the card, if any.
pub(super) fn select(
    archive: &AcceptedArchive,
    card: &CardSource,
) -> Result<Option<Claim>, Failure> {
    // The registered families each cover a single-face, single-line instant
    // or sorcery, read from an archive cut whose evidence contract they know.
    let (ArchiveCut::R5 | ArchiveCut::R6) = archive.cut else {
        return Ok(None);
    };
    let [(source, face)] = card.faces.as_slice() else {
        return Ok(None);
    };
    if face.lines.len() != 1 {
        return Ok(None);
    }
    let Ok(types) = type_line::parse(&source.type_line, false) else {
        return Ok(None);
    };
    if !matches!(types.kind, CardKind::Instant | CardKind::Sorcery) {
        return Ok(None);
    }
    let Ok(abilities) = decode::<Vec<ast::Ability>>(face.abilities.get(), "abilities") else {
        return Ok(None);
    };
    Ok(claim(FAMILIES, &abilities)?.map(|action| Claim { action, types }))
}

fn claim(
    families: &[(&str, Select)],
    abilities: &[ast::Ability],
) -> Result<Option<SourceAction>, Failure> {
    let mut claims = families
        .iter()
        .filter_map(|(name, select)| select(abilities).map(|action| (*name, action)));
    let Some((first, action)) = claims.next() else {
        return Ok(None);
    };
    if let Some((second, _)) = claims.next() {
        return Err(invalid(
            "compose.source_authority",
            format!("families {first} and {second} both claim the card"),
        ));
    }
    Ok(Some(action))
}

pub(super) fn compose(
    archive: &AcceptedArchive,
    card: &mut CardSource,
    claim: Claim,
    evidence: Option<&CardEvidence>,
) -> Result<Definition, Failure> {
    let Claim { action, types } = claim;
    let face = action.site().face;
    let (source, result) = card.faces.pop().unwrap();
    let printed: Value = decode(result.abilities.get(), "abilities")?;
    if let Some(field) = null_field(&printed, "abilities") {
        return Err(unsupported("operand", format!("operand:ast:null:{field}")));
    }
    let line = &result.lines[action.site().block];
    if printed != json!([decode::<Value>(line.ability.get(), "lines.ability")?])
        || source_lines(&source.oracle_text, &source.name, &source.type_line)?
            != [normalization::literal_spelling(&line.text)]
    {
        return Err(invalid(
            "lines",
            "source, line and aggregate ability disagree",
        ));
    }
    let evidence = evidence.ok_or_else(|| invalid("printed", "missing verified card evidence"))?;
    let checked = action.check(&source.oracle_text, evidence)?;
    let (mut definition, labels) = assemble::shell(archive, card, face, source, &result, types)?;
    checked.lower(&mut definition)?;
    keywords::printed(&card.shape, &[])?;
    definition.provenance.printed_sha256 = evidence.sha256().map(str::to_owned);
    let definition = assemble::finish(definition, labels);
    definition
        .validate()
        .map_err(|e| unsupported("composition", e))?;
    Ok(definition)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_families_claiming_one_card_is_a_registration_fault() {
        let abilities: Vec<ast::Ability> = serde_json::from_value(json!([{
            "kind": "spell",
            "effects": [{ "steps": [{
                "effect": "draw",
                "who": { "player": "you" },
                "amount": { "amount": "fixed", "value": 1 }
            }] }]
        }]))
        .unwrap();
        assert!(claim(FAMILIES, &abilities).unwrap().is_some());
        let doubled: &[(&str, Select)] = &[
            ("draw", source_action::draw),
            ("draw-again", source_action::draw),
        ];
        let fault = claim(doubled, &abilities).err().unwrap();
        assert!(!fault.unsupported);
        assert_eq!(
            fault.reason,
            "families draw and draw-again both claim the card"
        );
    }
}
