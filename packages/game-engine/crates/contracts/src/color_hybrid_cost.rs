//! Ordered choices for printed two-color hybrid and single-color Phyrexian symbols.
//! CR 107.4e-f, 119.4, 202.3d, 601.2b and 601.2f–h.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub enum HybridPayment {
    #[serde(rename = "pay_two_life")]
    PayTwoLife,
    #[serde(untagged)]
    Mana(Color),
}

impl HybridPayment {
    pub fn color(self) -> Option<Color> {
        match self {
            Self::Mana(color) => Some(color),
            Self::PayTwoLife => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ColorHybridSymbol {
    pub colors: [HybridPayment; 2],
}

pub fn color_hybrid_cost(text: &str) -> Option<(ManaCost, Vec<ColorHybridSymbol>)> {
    ordinary_cost(text).filter(|(_, hybrid)| !hybrid.is_empty())
}

/// A hybrid reminder is not a keyword block (CR 207.2a). Consume it
/// only when it describes one of this source's printed symbols.
pub fn hybrid_intrinsic_keywords(
    printed_cost: &str,
    text: &str,
    ast: &str,
) -> Result<Vec<Vec<KeywordAbility>>, String> {
    const INVALID: &str = "hybrid creature requires complete printed intrinsic keywords";
    let mut lines = text.lines().peekable();
    if let Some(&line) = lines.peek()
        && line.starts_with('(')
    {
        let exact = printed_cost.split('}').any(|part| {
            let Some((first, second)) = part.strip_prefix('{').and_then(|p| p.split_once('/'))
            else {
                return false;
            };
            let payment = if second == "P" {
                "2 life".to_string()
            } else {
                format!("{{{second}}}")
            };
            line == format!(
                "({{{first}/{second}}} can be paid with either {{{first}}} or {payment}.)"
            )
        });
        if !exact {
            return Err(INVALID.into());
        }
        lines.next();
    }
    let lines: Vec<_> = lines.collect();
    let ast: Vec<Value> = strict_read(ast)?;
    if lines.len() != ast.len() {
        return Err(INVALID.into());
    }
    let mut seen = BTreeSet::new();
    let mut blocks = Vec::new();
    for (printed, actual) in lines.into_iter().zip(ast) {
        let (words, reminder) = printed
            .split_once(" (")
            .map_or((printed, None), |(w, r)| (w, Some(format!("({r}"))));
        let mut block = Vec::new();
        let mut expected = Vec::new();
        for word in words.split(", ") {
            let word = word.to_ascii_lowercase();
            let keyword: KeywordAbility =
                serde_json::from_value(json!(word)).map_err(|_| INVALID)?;
            let (canonical, _) =
                fixed_creature_activation::intrinsic_keyword(keyword).ok_or(INVALID)?;
            if !seen.insert(canonical) {
                return Err(INVALID.into());
            }
            expected.push(json!({"keyword":canonical}));
            block.push(keyword);
        }
        let valid_reminder = match (block.as_slice(), reminder.as_deref()) {
            (_, None) => true,
            ([keyword], Some(text)) => fixed_creature_activation::keyword_reminder(*keyword)
                .is_some_and(|expected| text == expected),
            _ => false,
        };
        if !valid_reminder || actual != json!({"kind":"keywords","keywords":expected}) {
            return Err(INVALID.into());
        }
        blocks.push(block);
    }
    Ok(blocks)
}

pub(super) fn ordinary_cost(text: &str) -> Option<(ManaCost, Vec<ColorHybridSymbol>)> {
    fn color(symbol: &str) -> Option<Color> {
        match symbol {
            "W" => Some(Color::W),
            "U" => Some(Color::U),
            "B" => Some(Color::B),
            "R" => Some(Color::R),
            "G" => Some(Color::G),
            "C" => Some(Color::C),
            _ => None,
        }
    }
    let mut rest = text;
    let mut first = true;
    let mut base = ManaCost {
        generic: Quantity::zero(),
        colored: vec![],
    };
    let mut hybrid = vec![];
    while !rest.is_empty() {
        let (symbol, tail) = rest.strip_prefix('{')?.split_once('}')?;
        if let Some((a, b)) = symbol.split_once('/') {
            let a = color(a)?;
            if a == Color::C {
                return None;
            }
            let second = if b == "P" {
                HybridPayment::PayTwoLife
            } else {
                let b = color(b)?;
                if a == b || b == Color::C {
                    return None;
                }
                HybridPayment::Mana(b)
            };
            hybrid.push(ColorHybridSymbol {
                colors: [HybridPayment::Mana(a), second],
            });
        } else if let Some(c) = color(symbol) {
            base.colored.push(c);
        } else if first && !symbol.is_empty() && symbol.bytes().all(|b| b.is_ascii_digit()) {
            base.generic = symbol.parse().ok()?;
            if base.generic.to_string() != symbol {
                return None;
            }
        } else {
            return None;
        }
        rest = tail;
        first = false;
    }
    Some((base, hybrid))
}

pub(super) fn validate_definition(
    d: &Definition,
    opening_composition: bool,
    library_top: bool,
) -> Result<(), String> {
    let c = &d.characteristics;
    let raw = &d.provenance.source_json;
    if c.hybrid_mana.is_empty() && !raw.contains('/') {
        return Ok(());
    }
    let source: serde_json::Value = strict_read(raw)?;
    let printed = source["mana_cost"].as_str().unwrap_or("");
    let actual = color_hybrid_cost(printed);
    if actual.is_none() && c.hybrid_mana.is_empty() && !printed.contains('/') {
        return Ok(());
    }
    let text = source["oracle_text"].as_str().unwrap_or("");
    let intrinsic = !library_top && !opening_composition && !text.is_empty();
    let artifact_creature =
        intrinsic && c.kind == CardKind::Creature && c.additional_types == [CardKind::Artifact];
    let mut type_line = if artifact_creature {
        "Artifact Creature".to_string()
    } else {
        format!("{:?}", c.kind)
    };
    if library_top && c.legendary {
        type_line = format!("Legendary {type_line}");
    }
    if !c.subtypes.is_empty() {
        type_line.push_str(" — ");
        type_line.push_str(&c.subtypes.join(" "));
    }
    let printed_colors = c
        .mana_cost
        .colored
        .iter()
        .copied()
        .chain(
            c.hybrid_mana
                .iter()
                .flat_map(|s| s.colors)
                .filter_map(HybridPayment::color),
        )
        .filter(|c| *c != Color::C)
        .collect::<BTreeSet<_>>();
    let selected = Some((c.mana_cost.clone(), c.hybrid_mana.clone()));
    if (opening_composition
        && c.hybrid_mana
            .iter()
            .any(|s| s.colors.contains(&HybridPayment::PayTwoLife)))
        || actual != selected
        || source["oracle_id"] != d.oracle_id
        || source["id"] != d.provenance.printing_id
        || source["type_line"] != type_line
        || (c.legendary && !library_top)
        || c.snow
        || (!c.additional_types.is_empty() && !artifact_creature)
        || c.colors.iter().copied().collect::<BTreeSet<_>>() != printed_colors
        || c.color_identity.iter().copied().collect::<BTreeSet<_>>() != printed_colors
        || source["power"].as_str() != c.power.as_ref().map(ToString::to_string).as_deref()
        || source["toughness"].as_str() != c.toughness.as_ref().map(ToString::to_string).as_deref()
        || ["card_faces", "loyalty", "defense", "color_indicator"]
            .iter()
            .any(|k| !source[*k].is_null())
        || source["colors"] != serde_json::json!(c.colors)
        || source["color_identity"] != serde_json::json!(c.color_identity)
        || (!intrinsic
            && !library_top
            && !opening_composition
            && strict_read::<Value>(&d.provenance.abilities_json)? != json!([]))
        || !d.activated.is_empty()
        || (!library_top && !d.triggered.is_empty())
        || (!library_top && !opening_composition && !d.static_abilities.is_empty())
        || !d.mana_abilities.is_empty()
        || (!d.keywords.is_empty() && !intrinsic)
        || source["name"] != d.name
        || source["layout"] != "normal"
        || !matches!(
            c.kind,
            CardKind::Creature | CardKind::Artifact | CardKind::Enchantment
        )
        || d.x_mana_cost
        || c.basic
        || d.spell.as_ref().is_none_or(|p| {
            p.resolution != SpellResolution::Permanent
                || p.functioning_zone != FunctioningZone::Stack
                || p.timing
                    != if d.keywords.contains(&KeywordAbility::Flash) {
                        SpellTiming::Instant
                    } else {
                        SpellTiming::MainPhase
                    }
                || !p.targets.is_empty()
                || !p.modes.is_empty()
                || !p.instructions.is_empty()
                || p.additional_sacrifice.is_some()
                || !p.optional_additional_costs.is_empty()
        })
    {
        return Err(
            "color-hybrid symbols require a complete printed ordinary permanent cost".into(),
        );
    }
    if intrinsic {
        let blocks = hybrid_intrinsic_keywords(printed, text, &d.provenance.abilities_json)?;
        if c.kind != CardKind::Creature
            || !d.keywords.iter().copied().eq(blocks.into_iter().flatten())
        {
            return Err("hybrid creature requires complete printed intrinsic keywords".into());
        }
    }
    if c.hybrid_mana
        .iter()
        .flat_map(|symbol| symbol.colors)
        .filter_map(HybridPayment::color)
        .any(|color| !c.color_identity.contains(&color))
    {
        return Err("hybrid alternative outside printed color identity".into());
    }
    Ok(())
}

#[cfg(test)]
#[test]
fn color_hybrid_cost_separates_ordered_pairs_and_fixed_symbols() {
    let (cost, symbols) = color_hybrid_cost("{2}{G/W}{U}{B/R}").unwrap();
    assert_eq!(cost.generic, 2.into());
    assert_eq!(cost.colored, [Color::U]);
    assert_eq!(
        symbols,
        [
            ColorHybridSymbol {
                colors: [HybridPayment::Mana(Color::G), HybridPayment::Mana(Color::W)]
            },
            ColorHybridSymbol {
                colors: [HybridPayment::Mana(Color::B), HybridPayment::Mana(Color::R)]
            },
        ]
    );
    for invalid in [
        "",
        "{2}{U}",
        "{C/P}",
        "{2/G}",
        "{C/U}",
        "{U/U}",
        "{G/W}{X}",
        "{G/W}tail",
    ] {
        assert!(color_hybrid_cost(invalid).is_none(), "{invalid}");
    }
}

pub(super) fn extend_schema(schema: &mut serde_json::Value) {
    let phyrexian = json!({"contains":{"const":Capability::PhyrexianCostV1}});
    let symbol = json!({"properties":{"characteristics":{"required":["hybrid_mana"],"properties":{"hybrid_mana":{"contains":{"properties":{"colors":{"contains":{"const":"pay_two_life"}}}}}}}}});
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({"if":symbol,"then":{"properties":{"capabilities":phyrexian}},"else":{"properties":{"capabilities":{"not":phyrexian}}}}));
    let marker = json!({"required":["hybrid_mana"],"properties":{"hybrid_mana":{"minItems":1}}});
    let cap = json!({"contains":{"const":Capability::ColorHybridCostV1}});
    let keywords = json!({"required":["keywords"],"properties":{"keywords":{"minItems":1}}});
    let timing =
        |timing| json!({"properties":{"spell":{"properties":{"timing":{"const":timing}}}}});
    let intrinsic = json!({
        "properties":{
            "characteristics":{"properties":{"kind":{"const":"creature"},
                "additional_types":{"maxItems":1,"items":{"const":"artifact"}}}},
            "keywords":{"uniqueItems":true,"items":{"enum":[
                "flying","reach","trample","vigilance","defender","first strike","double strike",
                "flash","haste","menace","lifelink","deathtouch","hexproof","shroud","indestructible"
            ]}},
            "triggered":{"maxItems":0},"static_abilities":{"maxItems":0}
        },
        "if":{"properties":{"keywords":{"contains":{"const":"flash"}}}},
        "then":timing(SpellTiming::Instant),"else":timing(SpellTiming::MainPhase)
    });
    let ordinary = json!({"type":"object","properties":{
        "resolution":{"const":"permanent"},"functioning_zone":{"const":"stack"},
        "targets":{"maxItems":0},"instructions":{"maxItems":0},"modes":{"maxItems":0},
        "optional_additional_costs":{"maxItems":0},"additional_sacrifice":{"type":"null"}
    }});
    let shapes = [
        json!({"if":{"not":library_top_mana::marker_schema()},
            "then":{"properties":{"triggered":{"maxItems":0},"static_abilities":{"maxItems":0}}}}),
        json!({"if":keywords,"then":intrinsic,"else":timing(SpellTiming::MainPhase)}),
    ];
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "if":{"anyOf":[{"properties":{"characteristics":marker}},{"properties":{"capabilities":cap}},{"properties":{"provenance":{"properties":{"source_json":{"pattern":r#""mana_cost"\s*:\s*"[^"]*\{[WUBRG]/[WUBRGP]\}"#}}}}}]},
        "then":{"allOf":shapes,"required":["spell","capabilities","characteristics"],"properties":{
            "capabilities":cap,"characteristics":{"allOf":[marker],"properties":{
                "kind":{"enum":["artifact","creature","enchantment"]},
                "hybrid_mana":{"items":{"properties":{"colors":{
                    "uniqueItems":true,"prefixItems":[{"enum":["W","U","B","R","G"]}],
                    "items":{"enum":["W","U","B","R","G","pay_two_life"]}
                }}}}
            }},
            "spell":ordinary,"x_mana_cost":{"const":false},
            "activated":{"maxItems":0},"mana_abilities":{"maxItems":0}
        }}
    }));
}
