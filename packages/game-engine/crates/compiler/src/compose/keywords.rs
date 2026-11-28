//! Keyword table: evergreen keywords that are complete without parameters.
//!
//! CR 702.2-702.21 (pinned 2026-09-25). Cycling, kicker and flashback with
//! their reviewed cost shapes lower to their own rows below; other
//! parameterized or linked keywords (cascade, bestow) reject;
//! adding one here means adding one row and its runtime.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

/// CR 702.41, 601.2f: each printed instance contributes its own generic
/// reduction using the existing stack-static cost lock.
pub(super) fn affinity(keyword: &SourceKeyword, text: &str) -> Result<StaticAbility, Failure> {
    use kir::{
        Amount, Atom, PlayerExpr, Pred, Scope, SetExpr, SpellCostChange, StaticRule, ZoneExpr,
    };
    use serde_json::json;

    let quality = text
        .strip_prefix("Affinity for ")
        .ok_or_else(|| unsupported("keyword", "keyword:affinity:parameters"))?;
    let reject = || unsupported("keyword", format!("keyword:affinity:parameters:{quality}"));
    let filter = keyword.filter.as_ref().ok_or_else(reject)?;
    let atoms = match quality {
        "artifacts" if *filter == json!({"types":["artifact"]}) => {
            vec![Atom::Type {
                kind: CardKind::Artifact,
            }]
        }
        "creatures" if *filter == json!({"types":["creature"]}) => {
            vec![Atom::Type {
                kind: CardKind::Creature,
            }]
        }
        "artifact creatures" if *filter == json!({"types":["artifact","creature"]}) => {
            vec![
                Atom::Type {
                    kind: CardKind::Artifact,
                },
                Atom::Type {
                    kind: CardKind::Creature,
                },
            ]
        }
        "enchantments" if *filter == json!({"types":["enchantment"]}) => {
            vec![Atom::Type {
                kind: CardKind::Enchantment,
            }]
        }
        "tokens" if *filter == json!({"cls":"token"}) => vec![Atom::Token],
        "historic permanents"
            if *filter
                == json!({"cls":"permanent","allOf":[{"anyOf":[
                    {"supertypes":["legendary"]},{"types":["artifact"]},{"subtypes":["Saga"]}
                ]}]}) =>
        {
            vec![Atom::AnyOf {
                alternatives: vec![
                    Pred(vec![Atom::Legendary]),
                    Pred(vec![Atom::Type {
                        kind: CardKind::Artifact,
                    }]),
                    Pred(vec![Atom::Subtype {
                        subtype: "Saga".into(),
                    }]),
                ],
            }]
        }
        "outlaws"
            if *filter
                == json!({"cls":"permanent","allOf":[{"anyOf":[
                    {"subtypes":["Assassin"]},{"subtypes":["Mercenary"]},{"subtypes":["Pirate"]},
                    {"subtypes":["Rogue"]},{"subtypes":["Warlock"]}
                ]}]}) =>
        {
            vec![Atom::AnyOf {
                alternatives: ["Assassin", "Mercenary", "Pirate", "Rogue", "Warlock"]
                    .into_iter()
                    .map(|subtype| {
                        Pred(vec![Atom::Subtype {
                            subtype: subtype.into(),
                        }])
                    })
                    .collect(),
            }]
        }
        _ => {
            let subtype = match quality {
                "Equipment" => quality,
                "Allies" => "Ally",
                "Elves" => "Elf",
                "Swamps" => "Swamp",
                "Forests" => "Forest",
                "Mountains" => "Mountain",
                "Islands" => "Island",
                "Plains" => "Plains",
                "Gates" => "Gate",
                "Towns" => "Town",
                "Foods" => "Food",
                _ => quality
                    .strip_suffix('s')
                    .filter(|s| kir::is_creature_subtype(s))
                    .ok_or_else(reject)?,
            };
            if *filter != json!({"subtypes":[subtype]}) {
                return Err(reject());
            }
            vec![Atom::Subtype {
                subtype: subtype.into(),
            }]
        }
    };
    let caster = PlayerExpr::static_controller();
    let mut filter = Pred(atoms);
    filter.0.push(Atom::ControlledBy {
        player: caster.clone(),
    });
    Ok(StaticAbility {
        functioning_zone: FunctioningZone::Stack,
        effect: kir::StaticEffect::Rule {
            rule: StaticRule::SpellCost {
                affects: Scope::Source,
                caster,
                change: SpellCostChange::Reduce {
                    mana: ManaCost {
                        generic: 1.into(),
                        colored: vec![],
                    },
                },
                multiplier: Some(Amount::Count {
                    set: SetExpr::Matching {
                        zone: ZoneExpr::Battlefield,
                        filter,
                    },
                }),
            },
            condition: None,
        },
    })
}

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
    // CR 702.13, 702.28, 702.31, 702.36, 702.118: evasion abilities.
    ("intimidate", KeywordAbility::Intimidate),
    ("shadow", KeywordAbility::Shadow),
    ("horsemanship", KeywordAbility::Horsemanship),
    ("fear", KeywordAbility::Fear),
    ("skulk", KeywordAbility::Skulk),
];

/// CR 702.14a,c, 305.6: "[basic land type]walk" is landwalk for one basic
/// land subtype. Other landwalk qualities (nonbasic, snow, legendary, a card
/// type) reject.
const LANDWALK: &[(&str, KeywordAbility)] = &[
    ("Plains", KeywordAbility::Plainswalk),
    ("Island", KeywordAbility::Islandwalk),
    ("Swamp", KeywordAbility::Swampwalk),
    ("Mountain", KeywordAbility::Mountainwalk),
    ("Forest", KeywordAbility::Forestwalk),
];

fn landwalk(kind: CardKind, keyword: &SourceKeyword) -> Result<KeywordAbility, Failure> {
    let reject = || unsupported("keyword", "keyword:landwalk:quality");
    if kind != CardKind::Creature || keyword.cost.is_some() || keyword.protection_from.is_some() {
        return Err(reject());
    }
    let filter = keyword.filter.as_ref().ok_or_else(reject)?;
    basic_landwalk(filter).ok_or_else(reject)
}

pub(crate) fn basic_landwalk(filter: &serde_json::Value) -> Option<KeywordAbility> {
    let object = filter.as_object()?;
    if object.len() != 2 || object.get("types") != Some(&serde_json::json!(["land"])) {
        return None;
    }
    let [subtype] = object
        .get("subtypes")
        .and_then(|s| s.as_array())
        .map(Vec::as_slice)?
    else {
        return None;
    };
    let subtype = subtype.as_str()?;
    LANDWALK
        .iter()
        .find(|(name, _)| *name == subtype)
        .map(|(_, ability)| *ability)
}

/// The bare printed changeling keyword (CR 702.73a).
pub(super) fn changeling() -> SourceKeyword {
    SourceKeyword {
        keyword: "changeling".into(),
        cost: None,
        filter: None,
        protection_from: None,
        amount: None,
    }
}

pub(super) fn lower(
    kind: CardKind,
    keywords: &[SourceKeyword],
    text: &str,
) -> Result<Vec<KeywordAbility>, Failure> {
    lower_with_protection_text(kind, keywords, Some(text))
}

/// Companion evergreen/protection/landwalk keywords sharing a triggered-keyword
/// line; `triggered_printed` validates the entire line's token spelling.
pub(super) fn lower_triggered_companion(
    kind: CardKind,
    keywords: &[SourceKeyword],
) -> Result<Vec<KeywordAbility>, Failure> {
    lower_with_protection_text(kind, keywords, None)
}

fn lower_with_protection_text(
    kind: CardKind,
    keywords: &[SourceKeyword],
    protection_text: Option<&str>,
) -> Result<Vec<KeywordAbility>, Failure> {
    if keywords.is_empty() {
        return Err(unsupported("abilities.keywords", "empty keyword block"));
    }
    let mut lowered = Vec::new();
    for keyword in keywords {
        if keyword.keyword == "landwalk" {
            lowered.push(landwalk(kind, keyword)?);
            continue;
        }
        if keyword.cost.is_some() || keyword.filter.is_some() {
            return Err(unsupported(
                "keyword",
                format!("keyword:{}:parameters", keyword.keyword),
            ));
        }
        if let Some(from) = &keyword.protection_from {
            if keyword.keyword != "protection" {
                return Err(unsupported(
                    "keyword",
                    format!("keyword:{}:parameters", keyword.keyword),
                ));
            }
            lowered.extend(fixed_color_protection(kind, from, protection_text)?);
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
    text: Option<&str>,
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
    if let Some(text) = text {
        let text = text.to_ascii_lowercase();
        let clause = format!("protection from {phrase}");
        if text != clause && !text.ends_with(&format!(", {clause}")) {
            return Err(gap("printed_quality"));
        }
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
    check_printed(source, lowered, |_| false)
}

pub(super) fn printed_with_grants(
    source: &serde_json::Value,
    definition: &Definition,
) -> Result<(), Failure> {
    check_printed(source, &definition.keywords, |ability| {
        matches!(
            definition.characteristics.kind,
            CardKind::Instant | CardKind::Sorcery
        ) && definition
            .spell_programs()
            .flat_map(|program| &program.instructions)
            .any(|instruction| {
                matches!(instruction.verb(), Some(kir::Verb::SetCharacteristics { values, .. })
                    if values.keywords.contains(ability))
            })
    })
}

fn check_printed(
    source: &serde_json::Value,
    lowered: &[KeywordAbility],
    also_granted: impl Fn(&KeywordAbility) -> bool,
) -> Result<(), Failure> {
    let mismatch = || unsupported("source.keywords", "keyword list differs from the AST");
    let listed: BTreeSet<String> = source["keywords"]
        .as_array()
        .ok_or_else(mismatch)?
        .iter()
        .map(|k| k.as_str().map(str::to_lowercase).ok_or_else(mismatch))
        .collect::<Result<_, _>>()?;
    for (name, ability) in TABLE {
        let observed = listed.contains(*name);
        if observed == lowered.contains(ability) {
            continue;
        }
        // A spell's metadata can index a combat keyword its effect grants
        // (CR 113.10a, 611.2c). The exact lowered grant accounts for this
        // extra tag. A missing intrinsic keyword still fails the test.
        if observed && also_granted(ability) {
            continue;
        }
        return Err(mismatch());
    }
    if listed.contains("protection") != lowered.iter().any(|k| k.fixed_protection_color().is_some())
    {
        return Err(mismatch());
    }
    // Scryfall lists both "Landwalk" and the exact "[type]walk" keyword.
    let landwalks: BTreeSet<String> = lowered
        .iter()
        .filter_map(|k| LANDWALK.iter().find(|(_, a)| a == k))
        .map(|(name, _)| format!("{}walk", name.to_lowercase()))
        .collect();
    let listed_landwalks: BTreeSet<String> = listed
        .iter()
        .filter(|k| k.ends_with("walk") && *k != "landwalk")
        .cloned()
        .collect();
    if listed.contains("landwalk") != !landwalks.is_empty() || listed_landwalks != landwalks {
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

/// CR 702.122a: crew uses the stack and adds artifact and creature types
/// until cleanup. CR 301.7b supplies printed stats independently in layer 7.
pub(super) fn crew(
    card: &Card<'_>,
    keyword: &SourceKeyword,
    block: u32,
    text: &str,
) -> Result<ActivatedAbility, Failure> {
    let amount = keyword
        .amount
        .filter(|n| *n > 0 && text == format!("Crew {n}"))
        .filter(|_| card.kind == CardKind::Artifact && card.subtypes == ["Vehicle"])
        .ok_or_else(|| unsupported("keyword", "keyword:crew:parameters"))?;
    let mut program = SpellProgram::instructions(SpellTiming::Instant);
    program.instructions.push(Instruction::Kernel {
        source_block: block,
        node: kir::Node::Do(kir::Verb::SetCharacteristics {
            what: kir::SetExpr::One {
                object: kir::Ref::Source,
            },
            values: kir::CharacteristicSet {
                animation: Some(kir::Animation {
                    add_artifact: true,
                    replace_creature_subtypes: false,
                    creature_subtypes: vec![],
                }),
                ..Default::default()
            },
            until: kir::Until::EndOfTurn,
        }),
    });
    Ok(ActivatedAbility {
        x_mana_cost: false,
        hybrid_mana: vec![],
        modal: None,
        cost_changes: vec![],
        functioning_zone: FunctioningZone::Battlefield,
        restriction: None,
        cost: ActivationCost::ChooseObjects(ChosenObjectCost {
            action: ChosenCostAction::Tap,
            filter: ChosenCostFilter::Type(CardKind::Creature),
            count: 1,
            minimum_power: Some(amount),
            another: true,
            tap_source: false,
            mana: None,
        }),
        program,
    })
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
        x_mana_cost: false,
        hybrid_mana: vec![],
        modal: None,
        cost_changes: vec![],
        functioning_zone: FunctioningZone::Battlefield,
        restriction: None,
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
    if (keyword.keyword != "typecycling" && keyword.filter.is_some())
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
    program.instructions.push(Instruction::act(
        kir::Verb::Draw {
            player: kir::PlayerExpr::you(Context::Activated),
            amount: kir::Amount::fixed(1),
        },
        block,
    ));
    Ok(ActivatedAbility {
        x_mana_cost: false,
        hybrid_mana: vec![],
        modal: None,
        cost_changes: vec![],
        functioning_zone: FunctioningZone::Hand,
        restriction: None,
        cost: ActivationCost::DiscardSource { mana },
        program,
    })
}

/// CR 702.29e-f: typecycling uses the same hand cost as cycling. A
/// successful search reveals the card, then the library is always shuffled.
pub(super) fn typecycling(
    keyword: &SourceKeyword,
    block: u32,
    text: &str,
) -> Result<ActivatedAbility, Failure> {
    use kir::{Amount, Atom, Destination, Pred, SearchSpec, Verb};
    use serde_json::json;
    let reject = || unsupported("keyword", "keyword:typecycling:filter");
    let filter = keyword.filter.as_ref().ok_or_else(reject)?;
    let basic = json!({"types":["land"],"supertypes":["basic"]});
    let (name, atoms) = if *filter == basic {
        (
            "Basic landcycling".into(),
            vec![
                Atom::Type {
                    kind: CardKind::Land,
                },
                Atom::Basic,
            ],
        )
    } else {
        let subtype = [
            "Plains", "Island", "Swamp", "Mountain", "Forest", "Halfling", "Sliver", "Wizard",
        ]
        .into_iter()
        .find(|subtype| *filter == json!({"subtypes":[subtype]}))
        .ok_or_else(reject)?;
        (
            format!("{subtype}cycling"),
            vec![Atom::Subtype {
                subtype: subtype.into(),
            }],
        )
    };
    let mana = keyword_mana(keyword, &name, text)?;
    let player = kir::PlayerExpr::you(Context::Activated);
    let mut program = SpellProgram::instructions(SpellTiming::Instant);
    program.instructions = vec![
        Instruction::act(
            Verb::Search {
                player: player.clone(),
                search: SearchSpec {
                    filter: Pred(atoms),
                    maximum: Amount::Fixed { value: 1.into() },
                    to: Destination::OwnerHand,
                    reveal: true,
                    from: mtg_contracts::kir::SelectionZone::Library,
                    optional: false,
                    random: false,
                },
            },
            block,
        ),
        Instruction::act(Verb::Shuffle { player }, block),
    ];
    Ok(ActivatedAbility {
        x_mana_cost: false,
        hybrid_mana: vec![],
        modal: None,
        cost_changes: vec![],
        functioning_zone: FunctioningZone::Hand,
        restriction: None,
        cost: ActivationCost::DiscardSource { mana },
        program,
    })
}

/// CR 702.74a: "Evoke [cost]" is an alternative cost of this permanent
/// spell; its sacrifice trigger is [`TriggeredAbility::evoke_sacrifice`].
pub(super) fn evoke(
    keyword: &SourceKeyword,
    block: u32,
    text: &str,
) -> Result<OptionalAdditionalManaCost, Failure> {
    Ok(OptionalAdditionalManaCost {
        identity: mtg_contracts::EVOKE_COST.into(),
        source_block: block,
        mana: keyword_mana(keyword, "Evoke", text)?,
    })
}

/// CR 702.109a: "Dash [cost]" is an alternative cost of this creature
/// spell; its haste and end-step return follow from paying it.
pub(super) fn dash(
    keyword: &SourceKeyword,
    block: u32,
    text: &str,
) -> Result<OptionalAdditionalManaCost, Failure> {
    Ok(OptionalAdditionalManaCost {
        identity: mtg_contracts::DASH_COST.into(),
        source_block: block,
        mana: keyword_mana(keyword, "Dash", text)?,
    })
}

/// CR 702.30a: "Echo [cost]" is the upkeep trigger
/// [`TriggeredAbility::echo`] paying one fixed mana cost.
pub(super) fn echo(
    keyword: &SourceKeyword,
    block: u32,
    text: &str,
) -> Result<TriggeredAbility, Failure> {
    Ok(TriggeredAbility::echo(
        block,
        keyword_mana(keyword, "Echo", text)?,
    ))
}

/// CR 702.84a: "Unearth [cost]" is the graveyard ability
/// [`ActivatedAbility::unearth`] paying one fixed mana cost.
pub(super) fn unearth(
    keyword: &SourceKeyword,
    block: u32,
    text: &str,
) -> Result<ActivatedAbility, Failure> {
    Ok(ActivatedAbility::unearth(
        block,
        keyword_mana(keyword, "Unearth", text)?,
    ))
}

/// CR 702.141a: "Encore [cost]" is the graveyard ability
/// [`ActivatedAbility::encore`] paying one fixed mana cost.
pub(super) fn encore(
    keyword: &SourceKeyword,
    block: u32,
    text: &str,
) -> Result<ActivatedAbility, Failure> {
    Ok(ActivatedAbility::encore(
        block,
        keyword_mana(keyword, "Encore", text)?,
    ))
}

/// CR 702.138a: "Escape—[mana], Exile [N] other cards from your
/// graveyard.", printed exactly; other escape costs reject.
pub(super) fn escape(keyword: &SourceKeyword, text: &str) -> Result<EscapeCost, Failure> {
    let reject = || unsupported("keyword", "keyword:escape:cost");
    let [
        SourceCost::Mana { symbols },
        SourceCost::Exile { what, from: None },
    ] = keyword.cost.as_deref().unwrap_or_default()
    else {
        return Err(reject());
    };
    let words = [
        "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
    ];
    let count = what
        .pointer("/count/value")
        .and_then(serde_json::Value::as_u64)
        .and_then(|n| u32::try_from(n).ok())
        .filter(|n| (2..=10).contains(n))
        .ok_or_else(reject)?;
    let expected = serde_json::json!({
        "ref": "filter",
        "filter": {"other": true, "cls": "card", "zone": {"zone": "graveyard", "owner": "your"}},
        "count": {"amount": "fixed", "value": count},
    });
    if *what != expected
        || keyword.filter.is_some()
        || symbols.is_empty()
        || symbols
            .iter()
            .any(|s| s.is_empty() || s.contains(['{', '}']))
        || text
            != format!(
                "Escape\u{2014}{}, Exile {} other cards from your graveyard.",
                printed_mana(symbols),
                words[count as usize - 2]
            )
    {
        return Err(reject());
    }
    let mana = mana_cost(&printed_mana(symbols)).map_err(|_| reject())?;
    if mana.colored.contains(&Color::C) {
        return Err(reject());
    }
    Ok(EscapeCost {
        mana,
        exile_other: count,
    })
}

/// CR 702.34a: "Flashback [cost]" with one fixed mana cost.
pub(super) fn flashback(keyword: &SourceKeyword, text: &str) -> Result<ManaCost, Failure> {
    keyword_mana(keyword, "Flashback", text)
}

/// CR 702.37a-b: "Morph [cost]" or "Megamorph [cost]" with one fixed mana
/// cost. Nonmana morph costs (reveal, discard, life, return, sacrifice)
/// reject under `keyword:{name}:cost`.
pub(super) fn face_down(keyword: &SourceKeyword, text: &str) -> Result<FaceDownCast, Failure> {
    let (kind, name) = match keyword.keyword.as_str() {
        "morph" => (FaceDownKind::Morph, "Morph"),
        "megamorph" => (FaceDownKind::Megamorph, "Megamorph"),
        other => return Err(unsupported("keyword", format!("keyword:{other}"))),
    };
    Ok(FaceDownCast {
        kind,
        cost: keyword_mana(keyword, name, text)?,
    })
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

/// CR 702.27a: "Buyback [cost]" is an optional additional cost of this spell.
pub(super) fn buyback(
    keyword: &SourceKeyword,
    block: u32,
    text: &str,
) -> Result<OptionalAdditionalManaCost, Failure> {
    Ok(OptionalAdditionalManaCost {
        identity: mtg_contracts::BUYBACK_COST.into(),
        source_block: block,
        mana: keyword_mana(keyword, "Buyback", text)?,
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
                predicate: None,
            }),
        ));
    }
    Err(unsupported("keyword", "keyword:enchant:filter"))
}

/// Printed keyword abilities that are triggered abilities. Each instance
/// lowers to the complete trigger its rule defines and triggers separately
/// (CR 702.108b, 702.45b, 702.135b, 702.46b, 702.83b, 702.116b, 702.105b,
/// 702.149b, 702.130b, 702.23c, 702.121b, 702.91b, 702.25b, 702.86b,
/// 702.134b). The `bool` states
/// whether the keyword prints a fixed N.
const TRIGGERED: &[(&str, bool)] = &[
    ("prowess", false),
    ("bushido", true),
    ("afterlife", true),
    ("soulshift", true),
    ("exalted", false),
    ("myriad", false),
    ("dethrone", false),
    ("training", false),
    ("afflict", true),
    ("rampage", true),
    ("melee", false),
    ("battle cry", false),
    ("flanking", false),
    ("annihilator", true),
    ("mentor", false),
];

pub(super) fn is_triggered(keyword: &SourceKeyword) -> bool {
    TRIGGERED.iter().any(|(name, _)| *name == keyword.keyword)
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum LineToken {
    Word { value: String, possessive: bool },
    Number(String),
    Symbol,
    Punct(char),
}

/// Tokenize a normalized Oracle line using the same word/number/symbol
/// boundaries, horizontal-whitespace handling, and possessive flags as
/// `packages/oracle-parser/src/lexer.ts`. Notably letters and ASCII digits never
/// merge (`"afterlife1"` lexes to `afterlife`, `1`).
fn lex_line(line: &str) -> Vec<LineToken> {
    const CONTRACTIONS: &[&str] = &[
        "can't",
        "don't",
        "doesn't",
        "isn't",
        "aren't",
        "won't",
        "wasn't",
        "weren't",
        "hasn't",
        "haven't",
        "hadn't",
        "didn't",
        "couldn't",
        "shouldn't",
        "wouldn't",
        "it's",
        "that's",
        "there's",
        "they're",
        "you're",
        "you've",
        "he's",
        "she's",
        "what's",
        "who's",
        "let's",
    ];
    let chars: Vec<char> = line.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == ' ' || c == '\t' {
            i += 1;
            continue;
        }
        if c == '{'
            && let Some(rel) = chars[i + 1..].iter().position(|&d| d == '}')
        {
            tokens.push(LineToken::Symbol);
            i += rel + 2;
            continue;
        }
        if c.is_ascii_digit() {
            let start = i;
            while i < chars.len() && chars[i].is_ascii_digit() {
                i += 1;
            }
            tokens.push(LineToken::Number(chars[start..i].iter().collect()));
            continue;
        }
        if c.is_alphabetic() {
            let start = i;
            while i < chars.len() {
                let d = chars[i];
                if d.is_alphabetic()
                    || ((d == '\'' || d == '-')
                        && chars.get(i + 1).is_some_and(|n| n.is_alphabetic()))
                {
                    i += 1;
                } else {
                    break;
                }
            }
            let mut value: String = chars[start..i].iter().collect::<String>().to_lowercase();
            let mut possessive = false;
            if !CONTRACTIONS.contains(&value.as_str()) {
                if let Some(stem) = value.strip_suffix("'s") {
                    value = stem.to_owned();
                    possessive = true;
                } else if chars.get(i) == Some(&'\'') && value.ends_with('s') {
                    i += 1;
                    possessive = true;
                }
            }
            tokens.push(LineToken::Word { value, possessive });
            continue;
        }
        tokens.push(LineToken::Punct(c));
        i += 1;
    }
    tokens
}

fn consume_word<'a>(tokens: &'a [LineToken], expected: &str) -> Option<&'a [LineToken]> {
    match tokens {
        [
            LineToken::Word {
                value,
                possessive: false,
            },
            tail @ ..,
        ] if value == expected => Some(tail),
        _ => None,
    }
}

fn consume_words<'a>(mut tokens: &'a [LineToken], words: &[&str]) -> Option<&'a [LineToken]> {
    for word in words {
        tokens = consume_word(tokens, word)?;
    }
    Some(tokens)
}

/// Consume one inter-keyword separator matching `parseKeywordLine`
/// (`packages/oracle-parser/src/parser/keywords.ts:524-533`): `,`, `;`, `, and`,
/// `; and`, or bare non-possessive `and`.
fn consume_separator(tokens: &[LineToken]) -> Option<&[LineToken]> {
    match tokens {
        [
            LineToken::Punct(',' | ';'),
            LineToken::Word {
                value,
                possessive: false,
            },
            tail @ ..,
        ] if value == "and" => Some(tail),
        [LineToken::Punct(',' | ';'), tail @ ..] => Some(tail),
        [
            LineToken::Word {
                value,
                possessive: false,
            },
            tail @ ..,
        ] if value == "and" => Some(tail),
        _ => None,
    }
}

/// Consume one two-color protection separator matching `parseProtectionScopes`
/// (`packages/oracle-parser/src/parser/keywords.ts:248-252`): `, from`,
/// `, and from`, or `and from`.
fn consume_protection_separator(tokens: &[LineToken]) -> Option<&[LineToken]> {
    let after_sep = match tokens {
        [
            LineToken::Punct(','),
            LineToken::Word {
                value,
                possessive: false,
            },
            tail @ ..,
        ] if value == "and" => tail,
        [LineToken::Punct(','), tail @ ..] => tail,
        [
            LineToken::Word {
                value,
                possessive: false,
            },
            tail @ ..,
        ] if value == "and" => tail,
        _ => return None,
    };
    consume_word(after_sep, "from")
}

/// Consume one supported keyword token-for-token using the non-possessive
/// word/number rules of `packages/oracle-parser/src/parser/keywords.ts`.
fn consume_keyword<'a>(
    keyword: &SourceKeyword,
    tokens: &'a [LineToken],
) -> Option<&'a [LineToken]> {
    if is_triggered(keyword) {
        if keyword.cost.is_some() || keyword.filter.is_some() || keyword.protection_from.is_some() {
            return None;
        }
        let (name, parameterized) = TRIGGERED.iter().find(|(n, _)| *n == keyword.keyword)?;
        let words: Vec<&str> = name.split(' ').collect();
        let rest = consume_words(tokens, &words)?;
        return match (parameterized, keyword.amount) {
            (false, None) => Some(rest),
            (true, Some(n)) => match rest {
                [LineToken::Number(digits), tail @ ..] if *digits == n.to_string() => Some(tail),
                _ => None,
            },
            _ => None,
        };
    }
    if keyword.cost.is_some() || keyword.amount.is_some() {
        return None;
    }
    if keyword.keyword == "landwalk" {
        if keyword.protection_from.is_some() {
            return None;
        }
        let ability = basic_landwalk(keyword.filter.as_ref()?)?;
        let name = LANDWALK
            .iter()
            .find(|(_, a)| *a == ability)
            .map(|(name, _)| format!("{}walk", name.to_ascii_lowercase()))?;
        return consume_word(tokens, &name);
    }
    if keyword.filter.is_some() {
        return None;
    }
    if let Some(from) = &keyword.protection_from {
        if keyword.keyword != "protection" {
            return None;
        }
        use SourceProtectionQuality::{AllColors, Color};
        use mtg_contracts::fixed_color_protection::keyword_for_name;
        let rest = consume_words(tokens, &["protection", "from"])?;
        return match from.as_slice() {
            [AllColors {}] => consume_words(rest, &["each", "color"])
                .or_else(|| consume_words(rest, &["all", "colors"])),
            [Color { color }] if keyword_for_name(color).is_some() => {
                consume_word(rest, &color.to_ascii_lowercase())
            }
            [Color { color }, Color { color: other }]
                if color != other
                    && keyword_for_name(color).is_some()
                    && keyword_for_name(other).is_some() =>
            {
                let rest = consume_word(rest, &color.to_ascii_lowercase())?;
                let rest = consume_protection_separator(rest)?;
                consume_word(rest, &other.to_ascii_lowercase())
            }
            _ => None,
        };
    }
    let (name, _) = TABLE.iter().find(|(name, _)| *name == keyword.keyword)?;
    let mut rest = tokens;
    for part in name.split(' ') {
        rest = consume_word(rest, part)?;
    }
    Some(rest)
}

/// The keyword line prints each listed keyword left-to-right under the parser
/// token grammar (`lexer.ts` + `parseKeywordLine`) with at most one terminal
/// period, and every triggered instance matches its rule spelling and
/// `source["keywords"]` entry (CR 702.1).
pub(super) fn triggered_printed(
    keywords: &[SourceKeyword],
    text: &str,
    source: &serde_json::Value,
) -> Result<(), Failure> {
    let tokens = lex_line(text);
    let body = tokens
        .strip_suffix(&[LineToken::Punct('.')])
        .unwrap_or(tokens.as_slice());
    let listed: BTreeSet<String> = source["keywords"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|k| k.as_str().map(str::to_lowercase))
        .collect();
    let mut rest = body;
    for (index, keyword) in keywords.iter().enumerate() {
        let triggered = is_triggered(keyword);
        let reject = || {
            if triggered {
                unsupported("keyword", format!("keyword:{}:printed", keyword.keyword))
            } else {
                unsupported("keyword", "keyword:triggered:printed")
            }
        };
        if index > 0 {
            rest = consume_separator(rest)
                .ok_or_else(|| unsupported("keyword", "keyword:triggered:printed"))?;
        }
        rest = consume_keyword(keyword, rest).ok_or_else(reject)?;
        if triggered && !listed.contains(&keyword.keyword) {
            return Err(reject());
        }
    }
    if !rest.is_empty() {
        return Err(unsupported("keyword", "keyword:triggered:printed"));
    }
    Ok(())
}

/// CR 702.1, 113.2c: every occurrence of a reviewed triggered keyword anywhere
/// on a face's normalized lines (parser-aligned token scan across every block
/// kind) must be accounted for by the exact lowered AST keyword instance at the
/// same block, in order, with the same spelling.
pub(super) fn triggered_instances_printed(
    lines: &[String],
    abilities: &serde_json::Value,
) -> Result<(), Failure> {
    let mismatch = || unsupported("keyword", "keyword:triggered:printed_instances");
    let abilities = abilities.as_array().ok_or_else(mismatch)?;
    if abilities.len() != lines.len() {
        return Err(mismatch());
    }
    let mut printed = Vec::new();
    for (block, line) in lines.iter().enumerate() {
        let tokens = lex_line(line);
        let mut i = 0;
        while i < tokens.len() {
            // A name of several words ("battle cry") matches them in order.
            let found = TRIGGERED.iter().find_map(|(name, parameterized)| {
                let words: Vec<&str> = name.split(' ').collect();
                words
                    .iter()
                    .enumerate()
                    .all(|(k, w)| {
                        matches!(tokens.get(i + k), Some(LineToken::Word { value, .. }) if value == w)
                    })
                    .then_some((name, parameterized, words.len()))
            });
            if let Some((name, parameterized, width)) = found {
                let next = i + width;
                if *parameterized {
                    if let Some(LineToken::Number(n)) = tokens.get(next) {
                        printed.push((block, format!("{name} {n}")));
                        i = next + 1;
                        continue;
                    }
                    if let Some(LineToken::Word { value: w, .. }) = tokens.get(next)
                        && w == "x"
                    {
                        printed.push((block, format!("{name} x")));
                        i = next + 1;
                        continue;
                    }
                }
                printed.push((block, (*name).to_owned()));
                i = next;
                continue;
            }
            i += 1;
        }
    }
    let mut parsed = Vec::new();
    for (block, ability) in abilities.iter().enumerate() {
        if ability["kind"] != "keywords" {
            continue;
        }
        for keyword in ability["keywords"].as_array().ok_or_else(mismatch)? {
            let Some(name) = keyword["keyword"].as_str() else {
                return Err(mismatch());
            };
            if let Some((name, parameterized)) = TRIGGERED.iter().find(|(n, _)| *n == name) {
                let item = match (parameterized, keyword.get("amount")) {
                    (false, None) => (*name).to_owned(),
                    (true, Some(n)) if n.as_u64().is_some() => format!("{name} {n}"),
                    _ => return Err(mismatch()),
                };
                parsed.push((block, item));
            }
        }
    }
    if printed != parsed {
        return Err(mismatch());
    }
    Ok(())
}

/// CR 702.21a: the ward instances of a keyword block, decoded from the typed
/// AST and removed from the block the keyword mirror reads. The returned
/// block keeps every other keyword in order.
pub(super) fn split_ward(
    raw: &str,
) -> Result<Option<(String, Vec<ast::KeywordInstance>)>, Failure> {
    let reject = || unsupported("keyword", "keyword:ward:cost");
    let Ok(mut block) = serde_json::from_str::<serde_json::Value>(raw) else {
        return Ok(None);
    };
    if block["kind"] != "keywords" {
        return Ok(None);
    }
    let Some(list) = block
        .get_mut("keywords")
        .and_then(serde_json::Value::as_array_mut)
    else {
        return Ok(None);
    };
    if !list.iter().any(|k| k["keyword"] == "ward") {
        return Ok(None);
    }
    let mut wards = Vec::new();
    for keyword in list.iter().filter(|k| k["keyword"] == "ward") {
        wards.push(
            serde_json::from_value::<ast::KeywordInstance>(keyword.clone())
                .map_err(|_| reject())?,
        );
    }
    list.retain(|k| k["keyword"] != "ward");
    Ok(Some((block.to_string(), wards)))
}

/// CR 702.21a: "Ward [cost]" means "Whenever this permanent becomes the
/// target of a spell or ability an opponent controls, counter that spell or
/// ability unless that player pays [cost]." One fixed mana cost or "Pay N
/// life" lowers; any other cost rejects under `keyword:ward:cost`.
pub(super) fn ward(
    card: &Card<'_>,
    keyword: &ast::KeywordInstance,
    block: u32,
) -> Result<TriggeredAbility, Failure> {
    use kir::{Node, PlayerExpr, Pred, Ref, TargetingKind, Verb};
    if matches!(card.kind, CardKind::Instant | CardKind::Sorcery) {
        return Err(unsupported("keyword", "keyword:ward:source"));
    }
    let mut program = SpellProgram::instructions(SpellTiming::Instant);
    program.instructions.push(kernel(
        Node::UnlessPays {
            payer: PlayerExpr::ControllerOf {
                object: Ref::EventStackObject,
            },
            payment: ward_payment(keyword)?,
            node: Box::new(Node::Do(Verb::Counter {
                what: Ref::EventStackObject,
            })),
        },
        block,
    ));
    Ok(trigger_on(
        EventPattern::BecomesTarget {
            object: Pred::source(),
            by: PlayerExpr::EachOpponent,
            kind: TargetingKind::SpellOrAbility,
        },
        program,
    ))
}

/// The single printed ward cost and its spelling after "ward".
fn ward_cost(keyword: &ast::KeywordInstance) -> Result<(kir::Payment, String), Failure> {
    let reject = || unsupported("keyword", "keyword:ward:cost");
    let ast::KeywordInstance {
        keyword: _,
        cost: Some(cost),
        cost_modifications: None,
        restriction: None,
        amount: None,
        bindings: None,
        variable_constraints: None,
        from: None,
        filter: None,
        materials: None,
        companion: None,
        paired_with: None,
        enchant_player: None,
        prototype: None,
        additional_costs: None,
        gift: None,
    } = keyword
    else {
        return Err(reject());
    };
    match cost.as_slice() {
        [ast::Cost::Mana(mana)] if mana.multiplier.is_none() => {
            let symbols = &mana.symbols;
            if symbols.is_empty()
                || symbols
                    .iter()
                    .any(|s| s.is_empty() || s.contains(['{', '}']))
            {
                return Err(reject());
            }
            let printed = printed_mana(symbols);
            let cost = mana_cost(&printed).map_err(|_| reject())?;
            if cost.colored.contains(&Color::C) || cost.generic.is_negative() {
                return Err(reject());
            }
            let payment = if cost.colored.is_empty() {
                kir::Payment::Generic {
                    amount: kir::Amount::Fixed {
                        value: cost.generic,
                    },
                }
            } else {
                kir::Payment::Mana { mana: cost }
            };
            Ok((payment, format!(" {printed}")))
        }
        [ast::Cost::PayLife(life)] if life.multiplier.is_none() => {
            let ast::Amount::Fixed(n) = &*life.amount else {
                return Err(reject());
            };
            if n.value <= 0 {
                return Err(reject());
            }
            Ok((
                kir::Payment::Life {
                    amount: kir::Amount::fixed(n.value),
                },
                format!("\u{2014}pay {} life", n.value),
            ))
        }
        _ => Err(reject()),
    }
}

fn ward_payment(keyword: &ast::KeywordInstance) -> Result<kir::Payment, Failure> {
    ward_cost(keyword).map(|(payment, _)| payment)
}

/// The keyword line prints its keywords left to right, separated by
/// commas, each ward instance as "ward {cost}" or "ward—Pay N life" (alone
/// on its line), with at most one terminal period (CR 702.1, 702.21a).
/// `names` lists every keyword of the block in order, `None` for ward.
pub(super) fn ward_printed(
    names: &[Option<String>],
    wards: &[ast::KeywordInstance],
    text: &str,
) -> Result<(), Failure> {
    let reject = || unsupported("keyword", "keyword:ward:printed");
    let mut wards = wards.iter();
    let mut expected = String::new();
    for (index, name) in names.iter().enumerate() {
        let item = match name {
            Some(name) => name.clone(),
            None => {
                let (payment, spelling) = ward_cost(wards.next().ok_or_else(reject)?)?;
                if matches!(payment, kir::Payment::Life { .. }) && names.len() != 1 {
                    return Err(reject());
                }
                format!("ward{spelling}")
            }
        };
        if index > 0 {
            expected.push_str(", ");
        }
        expected.push_str(&item);
    }
    if wards.next().is_some() {
        return Err(reject());
    }
    let body = text.strip_suffix('.').unwrap_or(text);
    let mut chars = expected.chars();
    let expected: String = match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => return Err(reject()),
    };
    if body.to_lowercase() != expected.to_lowercase()
        || body.chars().next() != expected.chars().next()
    {
        return Err(reject());
    }
    Ok(())
}

fn trigger_on(pattern: EventPattern, program: SpellProgram) -> TriggeredAbility {
    TriggeredAbility {
        modal: None,
        functioning_zone: FunctioningZone::Battlefield,
        condition: TriggerCondition::Event {
            pattern,
            intervening_if: None,
            once_each_turn: false,
            your_turn: false,
        },
        program,
    }
}

/// One kernel step of a triggered program, labelled with its printed block.
fn kernel(node: kir::Node, block: u32) -> Instruction {
    Instruction::Kernel {
        source_block: block,
        node,
    }
}

/// "[What] gets +P/+T until end of turn" (CR 611.2a, 613.4c).
fn pump(what: kir::SetExpr, power: u32, toughness: u32) -> kir::Node {
    kir::Node::Do(kir::Verb::ModifyUntil {
        what,
        layer: kir::Layer::PowerToughnessModify,
        change: kir::Change::PowerToughness {
            power: kir::Amount::fixed(i64::from(power)),
            toughness: kir::Amount::fixed(i64::from(toughness)),
        },
        until: kir::Until::EndOfTurn,
    })
}

/// CR 608.2h, 611.2c: "[what] gets +X/+X until end of turn", X counted once.
fn counted_pump(what: kir::SetExpr, amount: kir::Amount) -> kir::Node {
    let delta = || kir::PumpDelta::Counted {
        amount: amount.clone(),
        negative: false,
    };
    kir::Node::Do(kir::Verb::ModifyUntil {
        what,
        layer: kir::Layer::PowerToughnessModify,
        change: kir::Change::PowerToughnessX {
            power: delta(),
            toughness: delta(),
        },
        until: kir::Until::EndOfTurn,
    })
}

/// "N for each [counted thing]" (CR 608.2h).
fn times(n: u32, of: kir::Amount) -> kir::Amount {
    if n == 1 {
        of
    } else {
        kir::Amount::Times {
            factor: Quantity::from(i64::from(n)),
            of: Box::new(of),
        }
    }
}

/// The complete defining trigger of one printed triggered keyword.
/// "This creature" forms apply to creature cards only; "this permanent"
/// forms to permanent cards. Pinned rules text:
/// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
pub(super) fn triggered(
    card: &Card<'_>,
    keyword: &SourceKeyword,
    block: u32,
) -> Result<TriggeredAbility, Failure> {
    use kir::{Atom, Node, Pred, Ref, SetExpr, Verb};
    let source = || unsupported("keyword", format!("keyword:{}:source", keyword.keyword));
    let creature = card.has_type(CardKind::Creature);
    let permanent = !matches!(card.kind, CardKind::Instant | CardKind::Sorcery);
    let n = keyword.amount.unwrap_or(1);
    let mut program = SpellProgram::instructions(SpellTiming::Instant);
    let this = SetExpr::One {
        object: Ref::Source,
    };
    let pattern = match keyword.keyword.as_str() {
        // CR 702.108a: "Whenever you cast a noncreature spell, this
        // creature gets +1/+1 until end of turn."
        "prowess" if creature => {
            program.instructions.push(kernel(pump(this, 1, 1), block));
            EventPattern::Cast {
                caster: PlayerExpr::you(Context::Triggered),
                spell: Pred(vec![Atom::NotType {
                    kind: CardKind::Creature,
                }]),
                nth: None,
                targets_source: false,
                target: Pred::default(),
            }
        }
        // CR 702.45a: "Whenever this creature blocks or becomes blocked, it
        // gets +N/+N until end of turn"; "it" is this creature.
        "bushido" if creature => {
            program.instructions.push(kernel(pump(this, n, n), block));
            EventPattern::AnyOf {
                first: Box::new(EventPattern::Blocks {
                    blocker: Pred::source(),
                }),
                second: Box::new(EventPattern::BecomesBlocked {
                    attacker: Pred::source(),
                }),
            }
        }
        // CR 702.135a: "When this permanent is put into a graveyard from the
        // battlefield, create N 1/1 white and black Spirit creature tokens
        // with flying."
        "afterlife" if permanent => {
            program.instructions.push(kernel(
                Node::Do(Verb::CreateTokens {
                    controller: PlayerExpr::you(Context::Triggered),
                    count: kir::Amount::fixed(i64::from(n)),
                    token: kir::TokenSpec::Creature {
                        spec: CreatureToken {
                            additional_types: vec![],
                            colors: vec![Color::W, Color::B],
                            subtypes: vec!["Spirit".into()],
                            all_creature_types: false,
                            power: Quantity::from(1),
                            toughness: Quantity::from(1),
                            keywords: vec![KeywordAbility::Flying],
                        },
                    },
                    tapped: false,
                }),
                block,
            ));
            EventPattern::Dies {
                object: Pred::source(),
            }
        }
        // CR 702.46a: "When this permanent is put into a graveyard from the
        // battlefield, you may return target Spirit card with mana value N
        // or less from your graveyard to your hand."
        "soulshift" if permanent => {
            program.targets.push(
                TargetSpec::new(0, "soulshift-card", TargetDomain::OwnGraveyardCard).with_filter(
                    PermanentFilter {
                        predicate: Some(Pred(vec![
                            Atom::Subtype {
                                subtype: "Spirit".into(),
                            },
                            Atom::ManaValue {
                                cmp: kir::Cmp::AtMost,
                                value: kir::Amount::fixed(i64::from(n)),
                            },
                        ])),
                        ..PermanentFilter::any()
                    },
                ),
            );
            program.instructions.push(kernel(
                Node::Optional {
                    chooser: PlayerExpr::you(Context::Triggered),
                    node: Box::new(Node::Do(Verb::Move {
                        what: SetExpr::One {
                            object: Ref::Target { slot: 0 },
                        },
                        to: kir::Destination::OwnerHand,
                    })),
                },
                block,
            ));
            EventPattern::Dies {
                object: Pred::source(),
            }
        }
        // CR 702.83a: "Whenever a creature you control attacks alone, that
        // creature gets +1/+1 until end of turn." CR 506.5: it attacks alone
        // when it is the only creature declared as an attacker.
        "exalted" if permanent => {
            program.instructions.push(kernel(
                pump(
                    SetExpr::One {
                        object: Ref::EventObject {
                            role: kir::EventObjectRole::Attacked,
                        },
                    },
                    1,
                    1,
                ),
                block,
            ));
            EventPattern::AttacksAlone {
                attacker: Pred(vec![
                    Atom::Type {
                        kind: CardKind::Creature,
                    },
                    Atom::ControlledBy {
                        player: PlayerExpr::you(Context::Triggered),
                    },
                ]),
            }
        }
        // CR 702.105a: "Whenever this creature attacks the player with the
        // most life or tied for most life, put a +1/+1 counter on this
        // creature." CR 702.149a: "Whenever this creature and at least one
        // other creature with power greater than this creature's power
        // attack, put a +1/+1 counter on this creature."
        "dethrone" | "training" if creature => {
            program.instructions.push(kernel(
                Node::Do(Verb::PutCounters {
                    on: this,
                    counter: kir::CounterKind::PlusOnePlusOne,
                    amount: kir::Amount::fixed(1),
                }),
                block,
            ));
            let attacker = Pred::source();
            if keyword.keyword == "dethrone" {
                EventPattern::AttacksPlayerWithMostLife { attacker }
            } else {
                EventPattern::AttacksWithGreaterPower { attacker }
            }
        }
        // CR 702.23a-b: "Whenever this creature becomes blocked, it gets
        // +N/+N until end of turn for each creature blocking it beyond the
        // first", counted once as the ability resolves.
        "rampage" if creature => {
            let beyond = kir::Amount::BlockersOf {
                object: Ref::Source,
                beyond: 1,
            };
            program
                .instructions
                .push(kernel(counted_pump(this, times(n, beyond)), block));
            EventPattern::BecomesBlocked {
                attacker: Pred::source(),
            }
        }
        // CR 702.91a: "Whenever this creature attacks, each other attacking
        // creature gets +1/+0 until end of turn"; the set is read as the
        // ability resolves (CR 608.2h, 611.2c).
        "battle cry" if creature => {
            program.instructions.push(kernel(
                Node::Do(Verb::ModifyUntil {
                    what: SetExpr::battlefield(Pred(vec![
                        Atom::Type {
                            kind: CardKind::Creature,
                        },
                        Atom::Attacking,
                        Atom::IsNot {
                            object: Ref::Source,
                        },
                    ])),
                    layer: kir::Layer::PowerToughnessModify,
                    change: kir::Change::PowerToughness {
                        power: kir::Amount::fixed(1),
                        toughness: kir::Amount::fixed(0),
                    },
                    until: kir::Until::EndOfTurn,
                }),
                block,
            ));
            EventPattern::Attacks {
                attacker: Pred::source(),
            }
        }
        // CR 702.121a: "Whenever this creature attacks, it gets +1/+1 until
        // end of turn for each opponent you attacked with a creature this
        // combat", counted as the ability resolves.
        "melee" if creature => {
            program.instructions.push(kernel(
                counted_pump(this, kir::Amount::OpponentsAttacked {}),
                block,
            ));
            EventPattern::Attacks {
                attacker: Pred::source(),
            }
        }
        // CR 702.130a: "Whenever this creature becomes blocked, defending
        // player loses N life."
        "afflict" if creature => {
            program.instructions.push(kernel(
                Node::Do(Verb::LoseLife {
                    player: PlayerExpr::event(kir::EventPlayerRole::DefendingPlayer),
                    amount: kir::Amount::fixed(i64::from(n)),
                }),
                block,
            ));
            EventPattern::BecomesBlocked {
                attacker: Pred::source(),
            }
        }
        // CR 702.86a: "Whenever this creature attacks, defending player
        // sacrifices N permanents"; they choose, and sacrifice every
        // permanent they control if they control fewer (CR 609.3). The
        // defending player is the one it attacks (CR 508.5).
        "annihilator" if creature => {
            program.instructions.push(kernel(
                Node::Do(Verb::Sacrifice {
                    player: PlayerExpr::event(kir::EventPlayerRole::DefendingPlayer),
                    count: kir::Amount::fixed(i64::from(n)),
                    filter: Pred::default(),
                    greatest_power: false,
                }),
                block,
            ));
            EventPattern::Attacks {
                attacker: Pred::source(),
            }
        }
        // CR 702.134a: "Whenever this creature attacks, put a +1/+1 counter
        // on target attacking creature with power less than this creature's
        // power." The bound is read at announcement and again at resolution,
        // from last known information once the source has left (CR 608.2b).
        "mentor" if creature => {
            program.targets.push(
                TargetSpec::new(0, "mentor-creature", TargetDomain::BattlefieldPermanent)
                    .with_filter(
                        PermanentFilter {
                            predicate: Some(Pred(vec![Atom::Attacking])),
                            ..PermanentFilter::any()
                        }
                        .with_types([CardKind::Creature])
                        .with_power(CreaturePowerFilter::LessThanSource {}),
                    ),
            );
            program.instructions.push(kernel(
                Node::Do(Verb::PutCounters {
                    on: SetExpr::target(0),
                    counter: kir::CounterKind::PlusOnePlusOne,
                    amount: kir::Amount::fixed(1),
                }),
                block,
            ));
            EventPattern::Attacks {
                attacker: Pred::source(),
            }
        }
        // CR 702.25a: the reviewed whole ability; its keyword marker is
        // added with it (`compose_face`).
        "flanking" if creature => return Ok(TriggeredAbility::flanking(block)),
        // CR 702.116a: the reviewed whole ability (tokens copy the card).
        "myriad" if creature => return Ok(TriggeredAbility::myriad(block)),
        _ => return Err(source()),
    };
    Ok(trigger_on(pattern, program))
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
                amount: None,
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
            amount: None,
        };
        assert!(enchant(&constrained, "Enchant creature").is_err());
    }
    #[test]
    fn affinity_rejects_unsupported_parameters_by_name() {
        use serde_json::json;
        for (quality, filter) in [
            (
                "snow lands",
                json!({"supertypes":["snow"],"types":["land"]}),
            ),
            ("planeswalkers", json!({"types":["planeswalker"]})),
            ("Affinity", json!({"keyword":"affinity"})),
        ] {
            let keyword = SourceKeyword {
                keyword: "affinity".into(),
                cost: None,
                filter: Some(filter),
                protection_from: None,
                amount: None,
            };
            let error = affinity(&keyword, &format!("Affinity for {quality}")).unwrap_err();
            assert!(error.unsupported);
            assert_eq!(
                error.reason,
                format!("keyword:affinity:parameters:{quality}")
            );
        }
    }
}
