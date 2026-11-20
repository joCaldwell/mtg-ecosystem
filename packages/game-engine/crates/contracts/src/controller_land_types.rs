//! CR 109.5, 205.3i, 305.6–7, 611.3 and 613.1d.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

pub fn controller_land_type_ast() -> Value {
    json!([{"kind":"static","effect":{"static":"modify",
        "what":{"ref":"each","filter":{"types":["land"],"control":"you"}},
        "modifiers":[{"modifier":"types","change":{"subtypes":[{"family":"land","operation":"add","types":{"all":true,"basic":true}}]}}]}}])
}

pub fn controller_nonland_colors_ast() -> Value {
    json!([{"kind":"static","effect":{"static":"modify","what":{"ref":"each","filter":{"nonTypes":["land"],"cls":"permanent","control":"you"}},"modifiers":[{"modifier":"colors","operation":"set","colors":["white","blue","black","red","green"]}]}}])
}

pub(super) fn complete_text(text: &str) -> bool {
    text.strip_prefix("Lands you control are ")
        .and_then(|s| s.strip_suffix(" in addition to their other types."))
        .is_some_and(|s| matches!(s, "every basic land type" | "all basic land types"))
}

pub(super) fn validate(d: &Definition) -> Result<(), String> {
    let has = d.static_abilities.iter().any(|a| {
        matches!(
            a.effect,
            StaticEffect::GrantControllerLandsAllBasicTypes
                | StaticEffect::ControllerNonlandsAllColors
        )
    });
    let raw = &d.provenance.source_json;
    if !has
        && !raw.contains("basic land type")
        && !raw.contains("all colors")
        && !raw.contains(r"\u")
    {
        return Ok(());
    }
    let source: Value = strict_read(raw)?;
    let text = source["oracle_text"].as_str().unwrap_or("");
    let is_colors = text == "Nonland permanents you control are all colors.";
    if !has && !complete_text(text) && !is_colors {
        return Ok(());
    }
    let c = &d.characteristics;
    let mana = &c.mana_cost;
    let mut printed = String::new();
    if mana.generic != Quantity::zero() || mana.colored.is_empty() {
        printed.push_str(&format!("{{{}}}", mana.generic));
    }
    for color in &mana.colored {
        printed.push_str(&format!("{{{color:?}}}"));
    }
    if (!complete_text(text) && !is_colors)
        || !has
        || source["name"] != d.name
        || source["oracle_id"] != d.oracle_id
        || source["id"] != d.provenance.printing_id
        || source["layout"] != "normal"
        || source["type_line"] != "Enchantment"
        || source["mana_cost"] != printed
        || source["colors"] != json!(c.colors)
        || source["color_identity"] != json!(c.color_identity)
        || ["card_faces", "loyalty", "defense", "power", "toughness"]
            .iter()
            .any(|key| !source[*key].is_null())
        || c.kind != CardKind::Enchantment
        || c.basic
        || c.legendary
        || c.snow
        || !c.subtypes.is_empty()
        || !c.additional_types.is_empty()
        || d.x_mana_cost
        || d.intrinsic_zone.is_some()
        || !d.activated.is_empty()
        || !d.triggered.is_empty()
        || !d.mana_abilities.is_empty()
        || !d.keywords.is_empty()
        || !matches!(
            d.static_abilities.as_slice(),
            [StaticAbility {
                functioning_zone: FunctioningZone::Battlefield,
                effect
            }] if *effect == (if is_colors {StaticEffect::ControllerNonlandsAllColors} else {StaticEffect::GrantControllerLandsAllBasicTypes})
        )
        || d.spell.as_ref().is_none_or(|p| {
            p.timing != SpellTiming::MainPhase
                || p.resolution != SpellResolution::Permanent
                || p.functioning_zone != FunctioningZone::Stack
                || !p.targets.is_empty()
                || !p.instructions.is_empty()
                || !p.modes.is_empty()
                || p.additional_sacrifice.is_some()
                || !p.optional_additional_costs.is_empty()
        })
        || strict_read::<Value>(&d.provenance.abilities_json)?
            != (if is_colors {
                controller_nonland_colors_ast()
            } else {
                controller_land_type_ast()
            })
        || d.source_labels.as_ref().is_some_and(|l| {
            l.text != text
                || !matches!(l.blocks.as_slice(),[block] if block==text)
                || l.statics != [0]
                || !l.activated.is_empty()
                || !l.triggered.is_empty()
                || !l.mana.is_empty()
                || !l.spell_modes.is_empty()
        })
    {
        return Err("unsupported complete controller characteristic grant source".into());
    }
    Ok(())
}
