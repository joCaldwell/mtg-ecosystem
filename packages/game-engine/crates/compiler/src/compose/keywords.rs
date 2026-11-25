//! Keyword table: evergreen keywords that are complete without parameters.
//!
//! CR 702.2-702.21 (pinned 2026-09-25). Cycling, kicker and flashback with
//! their reviewed cost shapes lower to their own rows below; other
//! parameterized or linked keywords (cascade, bestow, affinity) reject;
//! adding one here means adding one row and its runtime.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

const TABLE: &[(&str, KeywordAbility)] = &[
    ("flash", KeywordAbility::Flash),
    ("defender", KeywordAbility::Defender),
    ("trample", KeywordAbility::Trample),
    ("first strike", KeywordAbility::FirstStrike),
    ("double strike", KeywordAbility::DoubleStrike),
    ("menace", KeywordAbility::Menace),
    ("hexproof", KeywordAbility::Hexproof),
    ("shroud", KeywordAbility::Shroud),
    ("flying", KeywordAbility::Flying),
    ("reach", KeywordAbility::Reach),
    ("vigilance", KeywordAbility::Vigilance),
    ("haste", KeywordAbility::Haste),
    ("lifelink", KeywordAbility::Lifelink),
    ("deathtouch", KeywordAbility::Deathtouch),
    ("indestructible", KeywordAbility::Indestructible),
];

pub(super) fn lower(
    kind: CardKind,
    keywords: &[SourceKeyword],
    text: &str,
) -> Result<Vec<KeywordAbility>, Failure> {
    if keywords.is_empty() {
        return Err(unsupported("abilities.keywords", "empty keyword block"));
    }
    let mut lowered = Vec::new();
    for keyword in keywords {
        if keyword.cost.is_some() || keyword.filter.is_some() {
            return Err(unsupported(
                "keyword",
                format!("keyword:{}:parameters", keyword.keyword),
            ));
        }
        if let Some(from) = &keyword.protection_from {
            lowered.extend(fixed_color_protection(kind, from, text)?);
        } else {
            let ability = TABLE
                .iter()
                .find(|(name, _)| *name == keyword.keyword)
                .map(|(_, ability)| *ability)
                .ok_or_else(|| {
                    unsupported("abilities.keywords", format!("keyword:{}", keyword.keyword))
                })?;
            lowered.push(ability);
        }
    }
    Ok(lowered)
}

// CR 702.16a,g,h: each printed color is an independent protection ability.
fn fixed_color_protection(
    kind: CardKind,
    from: &[SourceProtectionQuality],
    text: &str,
) -> Result<Vec<KeywordAbility>, Failure> {
    use SourceProtectionQuality::{AllColors, Color};
    use mtg_contracts::fixed_color_protection::{KEYWORDS, keyword_for_name};
    let gap = |detail| unsupported("keyword", format!("keyword:protection:{detail}"));
    if kind != CardKind::Creature {
        return Err(gap("source"));
    }
    let (phrase, abilities) = match from {
        [AllColors {}] => ("each color".to_owned(), KEYWORDS.to_vec()),
        [Color { color }] => (
            color.clone(),
            vec![keyword_for_name(color).ok_or_else(|| gap("color"))?],
        ),
        [Color { color }, Color { color: other }] if color != other => (
            format!("{color} and from {other}"),
            vec![
                keyword_for_name(color).ok_or_else(|| gap("color"))?,
                keyword_for_name(other).ok_or_else(|| gap("color"))?,
            ],
        ),
        _ => return Err(gap("quality_list")),
    };
    let text = text.to_ascii_lowercase();
    let clause = format!("protection from {phrase}");
    if text != clause && !text.ends_with(&format!(", {clause}")) {
        return Err(gap("printed_quality"));
    }
    Ok(abilities)
}

/// The source's keyword list (Scryfall `keywords`) and the lowered intrinsic
/// keywords must name the same evergreen keywords. A source that lists Flash
/// while its AST has no flash block, or the reverse, is rejected rather than
/// given the wrong timing or combat abilities (CR 702.8, 702.9, 702.19).
pub(super) fn printed(
    source: &serde_json::Value,
    lowered: &[KeywordAbility],
) -> Result<(), Failure> {
    let mismatch = || unsupported("source.keywords", "keyword list differs from the AST");
    let listed: BTreeSet<String> = source["keywords"]
        .as_array()
        .ok_or_else(mismatch)?
        .iter()
        .map(|k| k.as_str().map(str::to_lowercase).ok_or_else(mismatch))
        .collect::<Result<_, _>>()?;
    for (name, ability) in TABLE {
        if listed.contains(*name) != lowered.contains(ability) {
            return Err(mismatch());
        }
    }
    if listed.contains("protection") != lowered.iter().any(|k| k.fixed_protection_color().is_some())
    {
        return Err(mismatch());
    }
    Ok(())
}

fn target(domain: TargetDomain, filter: Option<PermanentFilter>) -> TargetSpec {
    TargetSpec {
        slot: 0,
        selection_id: "attachment-host".into(),
        chooser: Chooser::Controller,
        stage: EvaluationStage::Announcement,
        domain,
        filter,
    }
}

// CR 702.6a: the implicit target and timing belong to the equip activation.
pub(super) fn equip(
    card: &Card<'_>,
    keyword: &SourceKeyword,
    block: u32,
    text: &str,
) -> Result<ActivatedAbility, Failure> {
    let costs = keyword.cost.as_deref().unwrap_or_default();
    let [SourceCost::Mana { symbols }] = costs else {
        return Err(unsupported("keyword", "keyword:equip:cost"));
    };
    let printed = symbols
        .iter()
        .map(|s| format!("{{{s}}}"))
        .collect::<String>();
    if text != format!("Equip {printed}") || keyword.filter.is_some() {
        return Err(unsupported("keyword", "keyword:equip:qualification"));
    }
    let cost = costs::lower(card, costs)?;
    let mut program = SpellProgram::instructions(SpellTiming::MainPhase);
    program
        .targets
        .push(target(TargetDomain::ControlledBattlefieldCreature, None));
    program.instructions.push(Instruction::Kernel {
        source_block: block,
        node: kir::Node::Do(kir::Verb::Attach {
            what: kir::Ref::Source,
            to: kir::Ref::Target { slot: 0 },
        }),
    });
    Ok(ActivatedAbility {
        functioning_zone: FunctioningZone::Battlefield,
        cost,
        program,
    })
}

/// The printed mana symbols of a keyword cost, as written.
fn printed_mana(symbols: &[String]) -> String {
    symbols.iter().map(|s| format!("{{{s}}}")).collect()
}

/// A keyword's single fixed mana cost, printed exactly as `"{Name} {cost}"`
/// (CR 702.29a, 702.33a, 702.34a). Hybrid, X, Phyrexian and nonmana costs
/// reject under `keyword:{name}:cost`.
fn keyword_mana(keyword: &SourceKeyword, name: &str, text: &str) -> Result<ManaCost, Failure> {
    let costs = keyword.cost.as_deref().unwrap_or_default();
    let reject = || unsupported("keyword", format!("keyword:{}:cost", keyword.keyword));
    let [SourceCost::Mana { symbols }] = costs else {
        return Err(reject());
    };
    if keyword.filter.is_some()
        || symbols.is_empty()
        || symbols
            .iter()
            .any(|s| s.is_empty() || s.contains(['{', '}']))
        || text != format!("{name} {}", printed_mana(symbols))
    {
        return Err(reject());
    }
    let mana = mana_cost(&printed_mana(symbols)).map_err(|_| reject())?;
    if mana.colored.contains(&Color::C) {
        return Err(reject());
    }
    Ok(mana)
}

/// CR 702.29a: "Cycling [cost]" means "[Cost], Discard this card: Draw a
/// card." It functions only while the card is in its owner's hand
/// (CR 113.6, 602.2a) and is activated at instant speed.
pub(super) fn cycling(
    keyword: &SourceKeyword,
    block: u32,
    text: &str,
) -> Result<ActivatedAbility, Failure> {
    let mana = keyword_mana(keyword, "Cycling", text)?;
    let mut program = SpellProgram::instructions(SpellTiming::Instant);
    program.instructions.push(Instruction::draw(
        kir::PlayerExpr::you(Context::Activated),
        Quantity::from(1),
        block,
    ));
    Ok(ActivatedAbility {
        functioning_zone: FunctioningZone::Hand,
        cost: ActivationCost::DiscardSource { mana },
        program,
    })
}

/// CR 702.34a: "Flashback [cost]" with one fixed mana cost.
pub(super) fn flashback(keyword: &SourceKeyword, text: &str) -> Result<ManaCost, Failure> {
    keyword_mana(keyword, "Flashback", text)
}

/// The one kicker identity a spell declares; "if this spell was kicked"
/// reads it (CR 702.33d).
pub(super) const KICKER: &str = "kicker";

/// CR 702.33a: "Kicker [cost]" is an optional additional cost of this spell.
pub(super) fn kicker(
    keyword: &SourceKeyword,
    block: u32,
    text: &str,
) -> Result<OptionalAdditionalManaCost, Failure> {
    Ok(OptionalAdditionalManaCost {
        identity: KICKER.into(),
        source_block: block,
        mana: keyword_mana(keyword, "Kicker", text)?,
    })
}

// CR 303.4a and 702.5: the spell target also records the ongoing enchant restriction.
pub(super) fn enchant(keyword: &SourceKeyword, text: &str) -> Result<TargetSpec, Failure> {
    let f = keyword
        .filter
        .as_ref()
        .ok_or_else(|| unsupported("keyword", "keyword:enchant:filter"))?;
    for (noun, kind) in [
        ("creature", Some(CardKind::Creature)),
        ("artifact", Some(CardKind::Artifact)),
        ("land", Some(CardKind::Land)),
        ("permanent", None),
    ] {
        if text != format!("Enchant {noun}") || keyword.cost.is_some() {
            continue;
        }
        if (kind.is_some() && *f != serde_json::json!({"types":[noun]}))
            || (kind.is_none() && *f != serde_json::json!({"cls":"permanent"}))
        {
            continue;
        }
        if kind == Some(CardKind::Creature) {
            return Ok(target(TargetDomain::BattlefieldCreature, None));
        }
        return Ok(target(
            TargetDomain::BattlefieldPermanent,
            Some(PermanentFilter {
                types: kind.into_iter().collect(),
                non_types: vec![],
                non_colors: vec![],
                opponent: false,
                tapped: false,
                power: None,
            }),
        ));
    }
    Err(unsupported("keyword", "keyword:enchant:filter"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn enchant_consumes_the_entire_printed_filter() {
        for (line, filter, domain, types) in [
            (
                "Enchant creature",
                json!({"types":["creature"]}),
                TargetDomain::BattlefieldCreature,
                None,
            ),
            (
                "Enchant artifact",
                json!({"types":["artifact"]}),
                TargetDomain::BattlefieldPermanent,
                Some(vec![CardKind::Artifact]),
            ),
            (
                "Enchant land",
                json!({"types":["land"]}),
                TargetDomain::BattlefieldPermanent,
                Some(vec![CardKind::Land]),
            ),
            (
                "Enchant permanent",
                json!({"cls":"permanent"}),
                TargetDomain::BattlefieldPermanent,
                Some(vec![]),
            ),
        ] {
            let keyword = SourceKeyword {
                keyword: "enchant".into(),
                cost: None,
                filter: Some(filter),
                protection_from: None,
            };
            let spec = enchant(&keyword, line).unwrap();
            assert_eq!(spec.domain, domain);
            assert_eq!(spec.filter.map(|f| f.types), types);
            assert!(enchant(&keyword, "Enchant player").is_err());
        }
        let constrained = SourceKeyword {
            keyword: "enchant".into(),
            cost: None,
            filter: Some(json!({"types":["creature"],"control":"you"})),
            protection_from: None,
        };
        assert!(enchant(&constrained, "Enchant creature").is_err());
    }
}
