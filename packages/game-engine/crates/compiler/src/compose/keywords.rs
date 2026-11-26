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
        modal: None,
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
    program.instructions.push(Instruction::draw(
        kir::PlayerExpr::you(Context::Activated),
        Quantity::from(1),
        block,
    ));
    Ok(ActivatedAbility {
        modal: None,
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
                },
            },
            block,
        ),
        Instruction::act(Verb::Shuffle { player }, block),
    ];
    Ok(ActivatedAbility {
        modal: None,
        functioning_zone: FunctioningZone::Hand,
        restriction: None,
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
/// (CR 702.108b, 702.45b, 702.135b, 702.46b). The `bool` states
/// whether the keyword prints a fixed N.
const TRIGGERED: &[(&str, bool)] = &[
    ("prowess", false),
    ("bushido", true),
    ("afterlife", true),
    ("soulshift", true),
];

pub(super) fn is_triggered(keyword: &SourceKeyword) -> bool {
    TRIGGERED.iter().any(|(name, _)| *name == keyword.keyword)
}

/// The printed spelling of one triggered keyword instance ("bushido 2").
fn spelling(keyword: &SourceKeyword) -> Option<String> {
    let (name, parameterized) = TRIGGERED.iter().find(|(n, _)| *n == keyword.keyword)?;
    match (parameterized, keyword.amount) {
        (false, None) => Some((*name).to_owned()),
        (true, Some(n)) => Some(format!("{name} {n}")),
        _ => None,
    }
}

/// Trim a normalized keyword line and strip one optional terminal period,
/// matching `parseKeywordLine` in `packages/oracle-parser/src/parser/keywords.ts`.
fn trim_keyword_line(line: &str) -> String {
    let trimmed = line.trim();
    trimmed
        .strip_suffix('.')
        .map_or(trimmed, str::trim)
        .to_ascii_lowercase()
}

/// Split one keyword line on commas, semicolons and standalone `and`
/// (including Oxford `, and` / `; and`) using the same item grammar as
/// `parseKeywordLine` (`packages/oracle-parser/src/parser/keywords.ts:524-535`).
fn keyword_line_items(line: &str) -> Vec<String> {
    let stripped = trim_keyword_line(line);
    if stripped.is_empty() {
        return Vec::new();
    }
    let mut items = Vec::new();
    for (chunk_idx, chunk) in stripped.split([',', ';']).enumerate() {
        let mut words = chunk.split_whitespace().peekable();
        if chunk_idx > 0 && words.peek().copied() == Some("and") {
            words.next();
        }
        let mut current = Vec::new();
        for word in words {
            if word == "and" {
                items.push(current.join(" "));
                current.clear();
            } else {
                current.push(word);
            }
        }
        items.push(current.join(" "));
    }
    items
}

/// The keyword line prints each listed keyword in order, separated by commas,
/// semicolons or `and` (with optional terminal period), and every triggered
/// instance exactly as its rule spells it with its N. Scryfall's keyword list
/// names each of them (CR 702.1).
pub(super) fn triggered_printed(
    keywords: &[SourceKeyword],
    text: &str,
    source: &serde_json::Value,
) -> Result<(), Failure> {
    let items = keyword_line_items(text);
    if items.len() != keywords.len() {
        return Err(unsupported("keyword", "keyword:triggered:printed"));
    }
    let listed: BTreeSet<String> = source["keywords"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|k| k.as_str().map(str::to_lowercase))
        .collect();
    for (keyword, item) in keywords.iter().zip(&items) {
        if !is_triggered(keyword) {
            continue;
        }
        let reject = || unsupported("keyword", format!("keyword:{}:printed", keyword.keyword));
        if keyword.cost.is_some()
            || keyword.filter.is_some()
            || keyword.protection_from.is_some()
            || spelling(keyword).as_ref() != Some(item)
            || !listed.contains(&keyword.keyword)
        {
            return Err(reject());
        }
    }
    Ok(())
}

/// The triggered keyword instance an item of a printed keyword line spells
/// ("bushido 2"), if it names one of the reviewed triggered keywords.
fn printed_instance(item: &str) -> Option<&'static str> {
    TRIGGERED
        .iter()
        .map(|(name, _)| *name)
        .find(|name| item == *name || item.strip_prefix(name).is_some_and(|n| n.starts_with(' ')))
}

/// CR 702.1, 113.2c: every triggered keyword instance printed on a face's
/// normalized lines is exactly one AST keyword instance of the same block, in
/// printed order, with the same spelling. An AST that drops, adds, reorders or
/// renumbers an instance rejects instead of losing or inventing a trigger.
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
    for (block, (line, ability)) in lines.iter().zip(abilities).enumerate() {
        if ability["kind"] == "keywords" {
            for item in keyword_line_items(line) {
                if !item.is_empty() && printed_instance(&item).is_some() {
                    printed.push((block, item));
                }
            }
        } else {
            let whole = trim_keyword_line(line);
            if printed_instance(&whole).is_some() {
                printed.push((block, whole));
            }
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

fn trigger_on(pattern: EventPattern, program: SpellProgram) -> TriggeredAbility {
    TriggeredAbility {
        modal: None,
        functioning_zone: FunctioningZone::Battlefield,
        condition: TriggerCondition::Event {
            pattern,
            intervening_if: None,
            once_each_turn: false,
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
}
