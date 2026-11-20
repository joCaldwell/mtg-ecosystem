//! CR 106.4, 500.5, 614.1a and 614.6: the controller's unspent
//! mana is retained in the replacement color instead of being lost.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

pub(super) fn text() -> String {
    format!(
        "If you would lose unspent mana, that mana becomes {} instead.",
        "black"
    )
}

fn abilities() -> Value {
    json!([{
        "kind":"static","effect":{"static":"replacement","replacement":{
            "event":{"event":"lose-unspent-mana","who":{"player":"you"}},
            "occurrence":"each","replace":{"replacement":"keep-mana","as":{"symbol":"B"}}
        }}
    }])
}

pub(super) fn validate(d: &Definition) -> Result<(), String> {
    let marked = d
        .static_abilities
        .iter()
        .any(|a| a.effect == StaticEffect::KeepUnspentManaAsBlack);
    let raw = &d.provenance.source_json;
    if !marked && !raw.contains("lose unspent mana") && !raw.contains(r"\u") {
        return Ok(());
    }
    let source: Value = strict_read(raw)?;
    let printed = source["oracle_text"].as_str().unwrap_or_default();
    let expected = text();
    if !marked && printed != expected {
        return Ok(());
    }
    let c = &d.characteristics;
    if printed != expected
        || strict_read::<Value>(&d.provenance.abilities_json)? != abilities()
        || source["layout"] != "normal"
        || source["id"] != d.provenance.printing_id
        || source["oracle_id"] != d.oracle_id
        || source["name"] != d.name
        || source["mana_cost"] != c.mana_cost.printed_fixed_cost()
        || source["colors"] != json!(c.colors)
        || source["color_identity"] != json!(c.color_identity)
        || c.kind != CardKind::Creature
        || !c.additional_types.is_empty()
        || c.basic
        || c.snow
        || !d.keywords.is_empty()
        || !d.activated.is_empty()
        || !d.triggered.is_empty()
        || !d.mana_abilities.is_empty()
        || d.intrinsic_zone.is_some()
        || d.x_mana_cost
        || d.static_abilities
            != [StaticAbility {
                functioning_zone: FunctioningZone::Battlefield,
                effect: StaticEffect::KeepUnspentManaAsBlack,
            }]
        || !matches!(d.spell.as_ref(), Some(p) if p.functioning_zone == FunctioningZone::Stack
            && p.timing == SpellTiming::MainPhase && p.resolution == SpellResolution::Permanent
            && p.targets.is_empty() && p.instructions.is_empty() && p.modes.is_empty()
            && p.additional_sacrifice.is_none() && p.optional_additional_costs.is_empty())
        || d.source_labels.as_ref().is_some_and(|l| {
            l.text != expected
                || l.blocks != [expected.clone()]
                || l.statics != [0]
                || !l.mana.is_empty()
                || !l.activated.is_empty()
                || !l.triggered.is_empty()
                || !l.spell_modes.is_empty()
        })
    {
        return Err("unspent mana conversion requires the complete controller permanent".into());
    }
    Ok(())
}

pub(super) fn extend_schema(schema: &mut Value) {
    let static_ability =
        json!({"functioning_zone":"battlefield","effect":"keep_unspent_mana_as_black"});
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "if":{"anyOf":[
            {"required":["static_abilities"],"properties":{"static_abilities":{"contains":{
                "required":["effect"],"properties":{"effect":{"const":"keep_unspent_mana_as_black"}}
            }}}},
            {"properties":{"provenance":{"properties":{"source_json":{
                "pattern":r#""oracle_text"\s*:\s*"If you would lose unspent mana, that mana becomes black instead\.""#
            }}}}}
        ]},
        "then":{"if":{"not":library_top_mana::marker_schema()},"then":{"properties":{"triggered":{"maxItems":0}}},"required":["spell","static_abilities"],"properties":{
            "characteristics":{"properties":{"kind":{"const":"creature"},"additional_types":{"maxItems":0},
                "basic":{"const":false},"snow":{"const":false}}},
            "static_abilities":{"const":[static_ability]},
            "capabilities":{"contains":{"const":Capability::UnspentManaConversionV1}},
            "spell":{"type":"object","properties":{"functioning_zone":{"const":"stack"},
                "timing":{"const":"main_phase"},"resolution":{"const":"permanent"},"targets":{"maxItems":0},
                "instructions":{"maxItems":0},"modes":{"maxItems":0},"additional_sacrifice":{"type":"null"},
                "optional_additional_costs":{"maxItems":0}}},
            "activated":{"maxItems":0},"mana_abilities":{"maxItems":0},
            "keywords":{"maxItems":0},"intrinsic_zone":{"type":"null"},"x_mana_cost":{"const":false}
        }},
        "else":{"properties":{"capabilities":{"not":{"contains":{"const":Capability::UnspentManaConversionV1}}}}}
    }));
}

#[cfg(test)]
pub(super) fn witness(mut d: Definition) -> Definition {
    let mut source: Value = strict_read(&d.provenance.source_json).unwrap();
    source["oracle_text"] = json!(text());
    d.source_labels = None;
    d.static_abilities.push(StaticAbility {
        functioning_zone: FunctioningZone::Battlefield,
        effect: StaticEffect::KeepUnspentManaAsBlack,
    });
    d.provenance.source_json = canonical(&source).unwrap();
    d.provenance.source_sha256 = sha256(d.provenance.source_json.as_bytes());
    d.provenance.abilities_json = canonical(&abilities()).unwrap();
    d.provenance.ast_sha256 = sha256(format!("[{}]", d.provenance.abilities_json).as_bytes());
    d.capabilities = d.requirements();
    d
}
