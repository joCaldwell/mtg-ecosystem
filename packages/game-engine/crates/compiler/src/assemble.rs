//! Assemble: the definition shell a printed card's characteristics determine
//! before any block lowers, and admission once every block has (CR 109.3,
//! 202, 208, 302.1, 304.1, 307.1, 608.3, 702.8).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

/// The characteristics, ordinary cast program and provenance of a card whose
/// blocks have not yet lowered, with its source labels.
pub(crate) fn shell(
    archive: &AcceptedArchive,
    r: Record,
    source: Source,
    result: &ParseResult,
    types: type_line::ParsedTypes,
    ast_sha256: String,
) -> Result<(Definition, SourceLabels), Failure> {
    let kind = types.kind;
    // CR 118.6: an absent mana cost is not {0}. Hybrid and X costs have no
    // block table row.
    if kind != CardKind::Land && source.mana_cost.is_empty() {
        return Err(unsupported(
            "source.mana_cost",
            "nonland without a mana cost requires an explicit absent-cost representation",
        ));
    }
    if source.mana_cost.contains("{X}") || color_hybrid_cost(&source.mana_cost).is_some() {
        return Err(unsupported(
            "source.mana_cost",
            "variable or hybrid costs outside the block tables",
        ));
    }
    let power = source
        .power
        .map(|s| s.parse::<Quantity>())
        .transpose()
        .map_err(|e| unsupported("source.power", e))?;
    let toughness = source
        .toughness
        .map(|s| s.parse::<Quantity>())
        .transpose()
        .map_err(|e| unsupported("source.toughness", e))?;
    let labels = SourceLabels {
        text: source.oracle_text.clone(),
        blocks: result.lines.iter().map(|l| l.text.clone()).collect(),
        ..SourceLabels::default()
    };
    let definition = Definition {
        source_labels: None,
        composed: true,
        x_mana_cost: false,
        oracle_id: r.oracle_identity,
        name: source.name,
        characteristics: Characteristics {
            kind,
            additional_types: types.additional_types,
            basic: types.basic,
            legendary: types.legendary,
            snow: types.snow,
            subtypes: types.subtypes,
            hybrid_mana: vec![],
            mana_cost: mana_cost(&source.mana_cost)?,
            colors: source.colors,
            color_identity: source.color_identity,
            power,
            toughness,
        },
        intrinsic_zone: (kind == CardKind::Land).then_some(FunctioningZone::Battlefield),
        spell: match kind {
            CardKind::Land => None,
            // CR 304.1, 307.1: ordinary timing follows the card type.
            CardKind::Instant => Some(SpellProgram::instructions(SpellTiming::Instant)),
            CardKind::Sorcery => Some(SpellProgram::instructions(SpellTiming::MainPhase)),
            // CR 302.1, 608.3: a permanent spell resolves onto the battlefield.
            _ => Some(SpellProgram::permanent(SpellTiming::MainPhase)),
        },
        activated: vec![],
        mana_abilities: vec![],
        triggered: vec![],
        static_abilities: vec![],
        keywords: vec![],
        capabilities: BTreeSet::new(),
        provenance: SourceProvenance {
            printing_id: source.id,
            source_index: r.source_index,
            source_sha256: r.source_sha256,
            ast_sha256,
            binding_sha256: r.binding_sha256,
            archive_manifest_sha256: archive.manifest_sha256.clone(),
            parser_sha256: archive.parser_source_sha256.clone(),
            source_json: r.source.get().into(),
            abilities_json: result.abilities.get().into(),
        },
    };
    Ok((definition, labels))
}

/// CR 702.8: Flash lets the spell be cast any time its controller could cast
/// an instant. The completed definition is admitted only by the composition
/// contract (`mtg_contracts::composition`).
pub(crate) fn admit(
    mut definition: Definition,
    labels: SourceLabels,
) -> Result<Definition, Failure> {
    if definition.keywords.contains(&KeywordAbility::Flash)
        && let Some(spell) = definition.spell.as_mut()
    {
        spell.timing = SpellTiming::Instant;
    }
    definition.source_labels = Some(labels);
    definition.capabilities = definition.requirements();
    definition
        .validate()
        .map_err(|e| unsupported("composition", e))?;
    Ok(definition)
}
