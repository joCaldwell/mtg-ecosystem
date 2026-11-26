//! Assemble: the definition shell a printed card's characteristics determine
//! before any block lowers, and admission once every block has (CR 109.3,
//! 202, 208, 302.1, 304.1, 307.1, 608.3, 702.8).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

/// The characteristics, ordinary cast program and provenance of a card whose
/// blocks have not yet lowered, with its source labels.
pub(crate) fn shell(
    archive: &AcceptedArchive,
    card: &CardSource,
    part: usize,
    source: SourceFace,
    result: &ParseResult,
    types: type_line::ParsedTypes,
) -> Result<(Definition, SourceLabels), Failure> {
    let r = &card.record;
    let kind = types.kind;
    // CR 712.8e, 202.3b: a transforming card's back face has no mana cost;
    // its mana value is set from the front face's once both are lowered.
    let transformed_back = card.layout == FaceLayout::Transforming && part == 1;
    if transformed_back != source.mana_cost.is_empty() && card.layout == FaceLayout::Transforming {
        return Err(unsupported(
            "source.mana_cost",
            "transforming card requires a front mana cost and a back face without one",
        ));
    }
    // CR 118.6: an absent mana cost is not {0}.
    if kind != CardKind::Land && source.mana_cost.is_empty() && !transformed_back {
        return Err(unsupported(
            "source.mana_cost",
            "nonland without a mana cost requires an explicit absent-cost representation",
        ));
    }
    let printed_cost = spell_cost(&source.mana_cost)?;
    let power = source
        .power
        .as_deref()
        .map(printed_stat)
        .transpose()
        .map_err(|e| unsupported("source.power", e))?
        .map(|(value, _)| value);
    let toughness = source
        .toughness
        .as_deref()
        .map(printed_stat)
        .transpose()
        .map_err(|e| unsupported("source.toughness", e))?
        .map(|(value, _)| value);
    // CR 202.2: an adventurer part's colors are those of its mana cost.
    let colors = match source.colors {
        Some(colors) => colors,
        None => cost_colors(&printed_cost),
    };
    let labels = SourceLabels {
        text: source.oracle_text.clone(),
        blocks: result.lines.iter().map(|l| l.text.clone()).collect(),
        ..SourceLabels::default()
    };
    let definition = Definition {
        layout: FaceLayout::Single,
        back_face: None,
        source_labels: None,
        composed: true,
        x_mana_cost: printed_cost.x,
        flashback: None,
        face_down: None,
        oracle_id: r.oracle_identity.clone(),
        name: source.name,
        characteristics: Characteristics {
            kind,
            additional_types: types.additional_types,
            basic: types.basic,
            legendary: types.legendary,
            snow: types.snow,
            subtypes: types.subtypes,
            all_creature_types: false,
            hybrid_mana: printed_cost.hybrid,
            mana_cost: printed_cost.base,
            colors,
            color_identity: card.source.color_identity.clone(),
            power,
            toughness,
            front_face_mana_value: None,
        },
        intrinsic_zone: (kind == CardKind::Land).then_some(FunctioningZone::Battlefield),
        modal_spell: None,
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
            printing_id: card.source.id.clone(),
            source_index: r.source_index,
            source_sha256: r.source_sha256.clone(),
            ast_sha256: card.ast_sha256.clone(),
            binding_sha256: r.binding_sha256.clone(),
            archive_manifest_sha256: archive.manifest_sha256.clone(),
            parser_sha256: archive.parser_source_sha256.clone(),
            source_json: r.source.get().into(),
            abilities_json: result.abilities.get().into(),
        },
    };
    Ok((definition, labels))
}

/// CR 208.2, 208.2a: a printed power or toughness and, when it includes a
/// star, the fixed number printed beside it. A star counts as zero until a
/// characteristic-defining ability defines the whole value (`*`, `N+*` and
/// `*+N`); any other spelling keeps its exact-quantity rejection.
pub(crate) fn printed_stat(text: &str) -> Result<(Quantity, Option<Quantity>), String> {
    let star = match text {
        "*" => Some("0"),
        _ => text
            .strip_suffix("+*")
            .or_else(|| text.strip_prefix("*+"))
            .filter(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit())),
    };
    match star {
        Some(n) => {
            let n = n.parse::<Quantity>()?;
            Ok((n.clone(), Some(n)))
        }
        None => Ok((text.parse::<Quantity>()?, None)),
    }
}

/// CR 208.2a, 604.3: a printed star is defined by exactly one
/// characteristic-defining ability, which defines nothing printed as a
/// number. `N+*` and `*+N` print the fixed term of that ability's sum.
pub(crate) fn stars_defined(
    statics: &[StaticAbility],
    stars: &[Option<Quantity>; 2],
) -> Result<(), Failure> {
    for (index, (field, star)) in ["source.power", "source.toughness"]
        .into_iter()
        .zip(stars)
        .enumerate()
    {
        let mut defined = statics.iter().filter_map(|ability| match &ability.effect {
            kir::StaticEffect::DefinePowerToughness { power, toughness } => {
                if index == 0 { power } else { toughness }.as_ref()
            }
            _ => None,
        });
        let amount = defined.next();
        if defined.next().is_some() {
            return Err(unsupported(
                field,
                "repeated characteristic-defining ability",
            ));
        }
        match (star, amount) {
            (None, None) => {}
            (Some(_), None) => {
                return Err(unsupported(
                    field,
                    "printed star without a characteristic-defining ability",
                ));
            }
            (None, Some(_)) => {
                return Err(unsupported(
                    field,
                    "characteristic-defining ability for a printed number",
                ));
            }
            (Some(fixed), Some(amount)) => {
                let printed_term = *fixed == Quantity::zero()
                    || matches!(amount, Amount::Sum { terms }
                        if terms.iter().filter(|t| t.as_fixed() == Some(fixed)).count() == 1
                            && terms.iter().filter(|t| t.as_fixed().is_some()).count() == 1);
                if !printed_term {
                    return Err(unsupported(
                        field,
                        "printed fixed term outside the characteristic-defining sum",
                    ));
                }
            }
        }
    }
    Ok(())
}

/// CR 105.2, 202.2: the colors of the mana symbols in a mana cost, in WUBRG
/// order; a hybrid symbol has each of its colors.
fn cost_colors(cost: &PrintedSpellCost) -> Vec<Color> {
    let mut colors: BTreeSet<Color> = cost.base.colored.iter().copied().collect();
    for symbol in &cost.hybrid {
        for payment in symbol.colors {
            if let HybridPayment::Mana(color) = payment {
                colors.insert(color);
            }
        }
    }
    colors.remove(&Color::C);
    [Color::W, Color::U, Color::B, Color::R, Color::G]
        .into_iter()
        .filter(|c| colors.contains(c))
        .collect()
}

/// CR 702.8: Flash lets the spell be cast any time its controller could cast
/// an instant. The completed definition is admitted only by the composition
/// contract (`mtg_contracts::composition`).
pub(crate) fn finish(mut definition: Definition, labels: SourceLabels) -> Definition {
    if definition.keywords.contains(&KeywordAbility::Flash)
        && let Some(spell) = definition.spell.as_mut()
    {
        spell.timing = SpellTiming::Instant;
    }
    definition.source_labels = Some(labels);
    definition.capabilities = definition.requirements();
    definition
}

/// A printed mana cost: its fixed symbols, its ordered two-color hybrid and
/// single-color Phyrexian symbols, and whether it begins with one `{X}`.
pub(crate) struct PrintedSpellCost {
    pub(crate) base: ManaCost,
    pub(crate) hybrid: Vec<ColorHybridSymbol>,
    pub(crate) x: bool,
}

/// CR 107.3, 107.4e-f, 202.1 and 601.2b/f: X is announced once while casting
/// and adds that much generic mana; each hybrid or Phyrexian symbol is paid
/// with one of its listed alternatives. A cost with several X symbols, a
/// monocolored or Phyrexian-hybrid symbol, or a colorless Phyrexian symbol
/// has no runtime cost and rejects.
pub(crate) fn spell_cost(text: &str) -> Result<PrintedSpellCost, Failure> {
    let (x, rest) = match text.strip_prefix("{X}") {
        Some(rest) => (true, rest),
        None => (false, text),
    };
    if rest.contains("{X}") {
        return Err(unsupported("source.mana_cost", "repeated or trailing X"));
    }
    if rest.contains('/') {
        return match color_hybrid_cost(rest) {
            Some((base, hybrid)) => Ok(PrintedSpellCost { base, hybrid, x }),
            None => Err(unsupported(
                "source.mana_cost",
                "variable/hybrid/Phyrexian/special mana is not supported",
            )),
        };
    }
    Ok(PrintedSpellCost {
        base: mana_cost(rest)?,
        hybrid: vec![],
        x,
    })
}
