//! A colored hybrid symbol is a mandatory choice between two colors.
//! CR 107.4e, 202.3d, 601.2b and 601.2f–h.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ColorHybridSymbol {
    pub colors: [Color; 2],
}

pub fn color_hybrid_cost(text: &str) -> Option<(ManaCost, Vec<ColorHybridSymbol>)> {
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
            let (a, b) = (color(a)?, color(b)?);
            if a == b || a == Color::C || b == Color::C {
                return None;
            }
            hybrid.push(ColorHybridSymbol { colors: [a, b] });
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
    (!hybrid.is_empty()).then_some((base, hybrid))
}

pub(super) fn validate_definition(d: &Definition, opening_composition: bool) -> Result<(), String> {
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
    let mut type_line = format!("{:?}", c.kind);
    if !c.subtypes.is_empty() {
        type_line.push_str(" — ");
        type_line.push_str(&c.subtypes.join(" "));
    }
    let printed_colors = c
        .mana_cost
        .colored
        .iter()
        .copied()
        .chain(c.hybrid_mana.iter().flat_map(|s| s.colors))
        .filter(|c| *c != Color::C)
        .collect::<BTreeSet<_>>();
    let selected = Some((c.mana_cost.clone(), c.hybrid_mana.clone()));
    if actual != selected
        || source["oracle_id"] != d.oracle_id
        || source["id"] != d.provenance.printing_id
        || source["type_line"] != type_line
        || c.legendary
        || c.snow
        || !c.additional_types.is_empty()
        || c.colors.iter().copied().collect::<BTreeSet<_>>() != printed_colors
        || c.color_identity.iter().copied().collect::<BTreeSet<_>>() != printed_colors
        || source["power"].as_str() != c.power.as_ref().map(ToString::to_string).as_deref()
        || source["toughness"].as_str() != c.toughness.as_ref().map(ToString::to_string).as_deref()
        || ["card_faces", "loyalty", "defense", "color_indicator"]
            .iter()
            .any(|k| !source[*k].is_null())
        || source["colors"] != serde_json::json!(c.colors)
        || source["color_identity"] != serde_json::json!(c.color_identity)
        || (!opening_composition && source["oracle_text"] != "")
        || (!opening_composition
            && strict_read::<serde_json::Value>(&d.provenance.abilities_json)?
                != serde_json::json!([]))
        || !d.activated.is_empty()
        || !d.triggered.is_empty()
        || (!opening_composition && !d.static_abilities.is_empty())
        || !d.mana_abilities.is_empty()
        || !d.keywords.is_empty()
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
    if c.hybrid_mana
        .iter()
        .flat_map(|symbol| symbol.colors)
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
                colors: [Color::G, Color::W]
            },
            ColorHybridSymbol {
                colors: [Color::B, Color::R]
            },
        ]
    );
    for invalid in [
        "",
        "{2}{U}",
        "{W/P}",
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
    use serde_json::json;
    let marker = json!({"required":["hybrid_mana"],"properties":{"hybrid_mana":{"minItems":1}}});
    let cap = json!({"contains":{"const":Capability::ColorHybridCostV1}});
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "if":{"anyOf":[{"properties":{"characteristics":marker}},{"properties":{"capabilities":cap}},{"properties":{"provenance":{"properties":{"source_json":{"pattern":r#""mana_cost"\s*:\s*"[^"]*\{[WUBRG]/[WUBRG]\}"#}}}}}]},
        "then":{"required":["spell","capabilities","characteristics"],"properties":{"capabilities":cap,"characteristics":{"allOf":[marker],"properties":{"kind":{"enum":["artifact","creature","enchantment"]},"hybrid_mana":{"items":{"properties":{"colors":{"uniqueItems":true,"items":{"enum":["W","U","B","R","G"]}}}}}}},"spell":{"type":"object","properties":{"resolution":{"const":"permanent"},"timing":{"const":"main_phase"},"functioning_zone":{"const":"stack"},"targets":{"maxItems":0},"instructions":{"maxItems":0},"modes":{"maxItems":0},"optional_additional_costs":{"maxItems":0},"additional_sacrifice":{"type":"null"}}},"x_mana_cost":{"const":false},"activated":{"maxItems":0},"triggered":{"maxItems":0},"static_abilities":{"maxItems":0},"mana_abilities":{"maxItems":0},"keywords":{"maxItems":0}}}
    }));
}
