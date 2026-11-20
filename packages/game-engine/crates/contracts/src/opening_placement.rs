//! CR 103.6a, 113.6, 400.7. The owner may place this exact kept hand card
//! before normal play, with the static ability functioning only from hand.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

pub fn opening_placement_ability() -> StaticAbility {
    StaticAbility {
        functioning_zone: FunctioningZone::Hand,
        effect: StaticEffect::MayBeginOnBattlefield,
    }
}

pub fn opening_placement_ast(name: &str, text: &str) -> Option<Value> {
    let (first, tail) = text
        .strip_prefix("If ")?
        .split_once(" is in your opening hand, you may begin the game with ")?;
    let second = tail.strip_suffix(" on the battlefield.")?;
    if second != "it" && second != name {
        return None;
    }
    let what = if first == "this card" {
        json!({"ref":"this","noun":"card"})
    } else if first == name {
        json!({"ref":"self"})
    } else {
        return None;
    };
    Some(
        json!([{"kind":"static","effect":{"static":"opening-hand-action","what":what,"who":{"player":"you"},"action":"begin-on-battlefield"}}]),
    )
}
pub(super) fn validate(d: &Definition) -> Result<(), String> {
    let has = d
        .static_abilities
        .iter()
        .any(|a| a.effect == StaticEffect::MayBeginOnBattlefield);
    let raw = &d.provenance.source_json;
    if !has && !raw.contains("opening hand") && !raw.contains(r"\u") {
        return Ok(());
    }
    let source: Value = strict_read(raw)?;
    let text = source["oracle_text"].as_str().unwrap_or("");
    let Some(ast) = opening_placement_ast(&d.name, text) else {
        return if has {
            Err("incomplete ordinary opening placement source".into())
        } else {
            Ok(())
        };
    };
    let c = &d.characteristics;
    let mut mana = String::new();
    if c.mana_cost.generic != Quantity::zero() || c.mana_cost.colored.is_empty() {
        mana.push_str(&format!("{{{}}}", c.mana_cost.generic));
    }
    for color in &c.mana_cost.colored {
        mana.push_str(&format!("{{{color:?}}}"));
    }
    if source["name"] != d.name
        || source["oracle_id"] != d.oracle_id
        || source["id"] != d.provenance.printing_id
        || source["layout"] != "normal"
        || source["type_line"] != "Enchantment"
        || source["mana_cost"] != mana
        || source["colors"] != json!(c.colors)
        || source["color_identity"] != json!(c.color_identity)
        || ["card_faces", "power", "toughness", "loyalty", "defense"]
            .iter()
            .any(|key| !source[*key].is_null())
        || c.kind != CardKind::Enchantment
        || c.basic
        || c.snow
        || c.legendary
        || !c.subtypes.is_empty()
        || !c.additional_types.is_empty()
        || c.power.is_some()
        || c.toughness.is_some()
        || d.x_mana_cost
        || d.intrinsic_zone.is_some()
        || !d.keywords.is_empty()
        || !d.activated.is_empty()
        || !d.triggered.is_empty()
        || !d.mana_abilities.is_empty()
        || d.static_abilities != [opening_placement_ability()]
        || d.spell.as_ref().is_none_or(|p| {
            p.timing != SpellTiming::MainPhase
                || p.functioning_zone != FunctioningZone::Stack
                || p.resolution != SpellResolution::Permanent
                || !p.instructions.is_empty()
                || !p.targets.is_empty()
                || !p.modes.is_empty()
                || !p.optional_additional_costs.is_empty()
                || p.additional_sacrifice.is_some()
        })
        || strict_read::<Value>(&d.provenance.abilities_json)? != ast
        || d.source_labels.as_ref().is_some_and(|l| {
            l.text != text
                || !matches!(l.blocks.as_slice(),[b] if b==&text.replace(&d.name,"~"))
                || l.statics != [0]
                || !l.triggered.is_empty()
                || !l.activated.is_empty()
                || !l.mana.is_empty()
                || !l.spell_modes.is_empty()
        })
    {
        return Err("opening placement requires its complete enchantment source".into());
    }
    Ok(())
}

pub(super) fn schema(schema: &mut Value) {
    let marker =
        json!({"properties":{"effect":{"const":"may_begin_on_battlefield"}},"required":["effect"]});
    let cap = Capability::OpeningPlacementV1;
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "if":{"anyOf":[{"required":["static_abilities"],"properties":{"static_abilities":{"contains":marker}}},
          {"properties":{"provenance":{"properties":{"source_json":{"pattern":r#""oracle_text"\s*:\s*"If [^"]+ is in your opening hand, you may begin the game with [^"]+ on the battlefield\.""#}}}}}]},
        "then":{"required":["static_abilities","spell"],"properties":{"static_abilities":{"const":[opening_placement_ability()]},"characteristics":{"properties":{"kind":{"const":"enchantment"},"basic":{"const":false},"snow":{"const":false},"legendary":{"const":false},"subtypes":{"maxItems":0},"additional_types":{"maxItems":0}}},
          "triggered":{"maxItems":0},"activated":{"maxItems":0},"mana_abilities":{"maxItems":0},"keywords":{"maxItems":0},"intrinsic_zone":{"type":"null"},"x_mana_cost":{"const":false},"spell":{"type":"object","properties":{"timing":{"const":"main_phase"},"resolution":{"const":"permanent"},"functioning_zone":{"const":"stack"},"instructions":{"maxItems":0},"targets":{"maxItems":0},"optional_additional_costs":{"maxItems":0},"additional_sacrifice":{"type":"null"},"modes":{"maxItems":0}}},
          "capabilities":{"uniqueItems":true,"minItems":3,"maxItems":3,"items":{"enum":[Capability::NormalLayoutV1,Capability::CastEnchantmentV1,cap]}}}},
        "else":{"properties":{"capabilities":{"not":{"contains":{"const":cap}}}}}
    }));
}
pub fn three_static_ast(name: &str, text: &str) -> Option<Value> {
    let mut lines = text.lines();
    let opening = opening_placement_ast(name, lines.next()?)?;
    if !matches!(
        lines.next()?,
        "Each nonland permanent you control is all colors."
            | "Nonland permanents you control are all colors."
    ) || !controller_land_types::complete_text(lines.next()?)
        || lines.next().is_some()
        || text.ends_with('\n')
    {
        return None;
    }
    Some(json!([
        opening[0],
        controller_nonland_colors_ast()[0],
        controller_land_type_ast()[0]
    ]))
}
fn three_statics() -> [StaticAbility; 3] {
    [
        opening_placement_ability(),
        StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect: StaticEffect::ControllerNonlandsAllColors,
        },
        StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect: StaticEffect::GrantControllerLandsAllBasicTypes,
        },
    ]
}
pub(super) fn validate_composition(d: &Definition) -> Result<bool, String> {
    let raw = &d.provenance.source_json;
    if !raw.contains("opening hand") && !raw.contains(r"\u") {
        return Ok(false);
    }
    let source: Value = strict_read(raw)?;
    let text = source["oracle_text"].as_str().unwrap_or("");
    let Some(ast) = three_static_ast(&d.name, text) else {
        return Ok(false);
    };
    let c = &d.characteristics;
    if d.static_abilities != three_statics()
        || c.hybrid_mana.is_empty()
        || c.kind != CardKind::Enchantment
        || !c.subtypes.is_empty()
        || c.power.is_some()
        || c.toughness.is_some()
        || d.intrinsic_zone.is_some()
        || strict_read::<Value>(&d.provenance.abilities_json)? != ast
        || d.source_labels.as_ref().is_some_and(|l| {
            l.text != text
                || l.blocks
                    != text
                        .lines()
                        .map(|l| l.replace(&d.name, "~"))
                        .collect::<Vec<_>>()
                || l.statics != [0, 1, 2]
                || !l.activated.is_empty()
                || !l.triggered.is_empty()
                || !l.mana.is_empty()
                || !l.spell_modes.is_empty()
        })
    {
        return Err("incomplete opening and controller characteristic composition".into());
    }
    Ok(true)
}
fn composition_shape() -> Value {
    let caps = [
        Capability::NormalLayoutV1,
        Capability::CastEnchantmentV1,
        Capability::ColorHybridCostV1,
        Capability::OpeningPlacementV1,
        Capability::ControllerNonlandColorsV1,
        Capability::ControllerLandBasicTypesV1,
    ];
    json!({"required":["spell","static_abilities","capabilities","characteristics"],"properties":{"static_abilities":{"const":three_statics()},"capabilities":{"minItems":6,"maxItems":6,"uniqueItems":true,"items":{"enum":caps}},"characteristics":{"required":["hybrid_mana"],"properties":{"kind":{"const":"enchantment"},"hybrid_mana":{"minItems":1},"basic":{"const":false},"legendary":{"const":false},"snow":{"const":false},"subtypes":{"maxItems":0},"additional_types":{"maxItems":0}}},"activated":{"maxItems":0},"triggered":{"maxItems":0},"mana_abilities":{"maxItems":0},"keywords":{"maxItems":0},"spell":{"type":"object","properties":{"timing":{"const":"main_phase"},"resolution":{"const":"permanent"},"functioning_zone":{"const":"stack"},"targets":{"maxItems":0},"instructions":{"maxItems":0},"modes":{"maxItems":0},"optional_additional_costs":{"maxItems":0},"additional_sacrifice":{"type":"null"}}}}})
}
pub(super) fn permit_composition(schema: &mut Value) {
    let entry = schema["$defs"]["Definition"]["allOf"]
        .as_array_mut()
        .unwrap()
        .last_mut()
        .unwrap();
    let original = entry["then"].take();
    entry["then"] = json!({"anyOf":[original,{"allOf":[composition_shape(),source_marker()]}]});
}
fn source_marker() -> Value {
    json!({"properties":{"provenance":{"properties":{"source_json":{"pattern":r#""oracle_text"\s*:\s*"If [^"]+ is in your opening hand,[^"]+ all colors\.[^"]+basic land types? in addition to their other types\.""#}}}}})
}
pub(super) fn require_composition(schema: &mut Value) {
    schema["$defs"]["Definition"]["allOf"]
        .as_array_mut()
        .unwrap()
        .push(json!({"if":source_marker(),"then":composition_shape()}));
}
