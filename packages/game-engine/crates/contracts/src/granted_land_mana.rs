//! CR 113.10, 302.6, 605.1, 605.3 and 611.3: the affected land
//! is the source of its granted tap ability, not the granting artifact.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

pub fn granted_land_mana_static() -> StaticAbility {
    StaticAbility {
        functioning_zone: FunctioningZone::Battlefield,
        effect: StaticEffect::GrantControllerLandsAnyColorMana,
    }
}

pub fn granted_land_mana_printed() -> ManaAbility {
    ManaAbility {
        source_block: 1,
        costs: PermanentActivationCosts {
            tap: true,
            mana: None,
            sacrifice: false,
            remove_source_plus_one_counters: None,
        },
        production: ManaProduction::AnyColor,
        restriction: None,
        self_damage: None,
    }
}

pub fn granted_land_mana_source(text: &str) -> Option<[Value; 2]> {
    let mut lines = text.lines();
    let quoted = lines
        .next()?
        .strip_prefix("Lands you control have \"")?
        .strip_suffix('"')?;
    if lines.next() != Some(quoted)
        || quoted.strip_prefix("{T}: Add ")? != "one mana of any color."
        || lines.next().is_some()
    {
        return None;
    }
    let mana = json!({"kind":"activated","costs":[{"cost":"tap-self"}],
        "effects":[{"steps":[{"effect":"add-mana","who":{"player":"you"},
            "mana":{"mana":"any-color","amount":{"amount":"fixed","value":1}}}]}]});
    Some([
        json!({"kind":"static","effect":{"static":"modify",
            "what":{"ref":"each","filter":{"types":["land"],"control":"you"}},
            "modifiers":[{"modifier":"abilities","abilities":[{"gained":"quoted","ability":mana}]}]}}),
        mana,
    ])
}

pub(super) fn validate_definition(d: &Definition) -> Result<(), String> {
    let marked = d
        .static_abilities
        .iter()
        .any(|a| a.effect == StaticEffect::GrantControllerLandsAnyColorMana);
    let raw = &d.provenance.source_json;
    if !marked && !raw.contains("Lands you control have") && !raw.contains(r"\u") {
        return Ok(());
    }
    let source: Value = strict_read(raw)?;
    let text = source["oracle_text"].as_str().unwrap_or("");
    let Some(expected) = granted_land_mana_source(text) else {
        return if marked {
            Err("invalid granted land mana source".into())
        } else {
            Ok(())
        };
    };
    let c = &d.characteristics;
    let mut cost = String::new();
    if c.mana_cost.generic != Quantity::zero() || c.mana_cost.colored.is_empty() {
        cost.push_str(&format!("{{{}}}", c.mana_cost.generic));
    }
    for color in &c.mana_cost.colored {
        cost.push_str(&format!("{{{color:?}}}"));
    }
    if source["layout"] != "normal"
        || source["type_line"] != "Artifact"
        || source["id"] != d.provenance.printing_id
        || source["oracle_id"] != d.oracle_id
        || source["name"] != d.name
        || source["mana_cost"] != cost
        || source["colors"] != json!(c.colors)
        || source["color_identity"] != json!(c.color_identity)
        || c.kind != CardKind::Artifact
        || c.basic
        || c.legendary
        || c.snow
        || c.power.is_some()
        || c.toughness.is_some()
        || !c.subtypes.is_empty()
        || !c.additional_types.is_empty()
        || d.x_mana_cost
        || d.intrinsic_zone.is_some()
        || !d.keywords.is_empty()
        || !d.activated.is_empty()
        || !d.triggered.is_empty()
        || !matches!(d.spell.as_ref(), Some(p) if p.functioning_zone == FunctioningZone::Stack
            && p.timing == SpellTiming::MainPhase && p.resolution == SpellResolution::Permanent
            && p.targets.is_empty() && p.instructions.is_empty() && p.modes.is_empty()
            && p.additional_sacrifice.is_none() && p.optional_additional_costs.is_empty())
        || d.static_abilities != [granted_land_mana_static()]
        || d.mana_abilities != [granted_land_mana_printed()]
        || strict_read::<[Value; 2]>(&d.provenance.abilities_json)? != expected
        || d.source_labels.as_ref().is_some_and(|l| {
            l.text != text
                || !text.lines().eq(l.blocks.iter().map(String::as_str))
                || l.statics != [0]
                || l.mana != [1]
                || !l.triggered.is_empty()
                || !l.activated.is_empty()
                || !l.spell_modes.is_empty()
        })
    {
        return Err("granted land mana requires both complete artifact abilities".into());
    }
    Ok(())
}

pub(super) fn extend_schema(schema: &mut Value) {
    let static_ability = granted_land_mana_static();
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "if":{"anyOf":[
            {"properties":{"static_abilities":{"contains":{"required":["effect"],
                "properties":{"effect":{"const":static_ability.effect}}}}}},
            {"properties":{"provenance":{"properties":{"source_json":{"pattern":
                r#""oracle_text"\s*:\s*"Lands you control have \\"\{T\}: Add one mana of any color\."#
            }}}}}
        ]},
        "then":{"required":["static_abilities","mana_abilities","spell"],"properties":{
            "characteristics":{"properties":{"kind":{"const":"artifact"},"basic":{"const":false},
                "legendary":{"const":false},"snow":{"const":false},"power":{"type":"null"},"toughness":{"type":"null"},
                "additional_types":{"maxItems":0},"subtypes":{"maxItems":0}}},
            "static_abilities":{"const":[static_ability]},
            "mana_abilities":{"const":[granted_land_mana_printed()]},
            "spell":{"type":"object","properties":{"timing":{"const":"main_phase"},
                "functioning_zone":{"const":"stack"},"resolution":{"const":"permanent"},
                "targets":{"maxItems":0},"instructions":{"maxItems":0},"modes":{"maxItems":0},
                "additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0}}},
            "activated":{"maxItems":0},"triggered":{"maxItems":0},"keywords":{"maxItems":0},
            "intrinsic_zone":{"type":"null"},"x_mana_cost":{"const":false},
            "capabilities":{"contains":{"const":Capability::ControllerLandGrantedManaV1}}
        }},
        "else":{"properties":{"capabilities":{"not":{"contains":{"const":Capability::ControllerLandGrantedManaV1}}}}}
    }));
}

#[cfg(test)]
pub(super) fn witness(mut definition: Definition) -> Definition {
    let mut source: Value = strict_read(&definition.provenance.source_json).unwrap();
    let printed = format!("{{T}}: Add {}.", "one mana of any color");
    let text = format!("Lands you control have \"{printed}\"\n{printed}");
    source["oracle_text"] = json!(text);
    definition.source_labels = None;
    definition.provenance.source_json = canonical(&source).unwrap();
    definition.provenance.source_sha256 = sha256(definition.provenance.source_json.as_bytes());
    definition.provenance.abilities_json =
        canonical(&granted_land_mana_source(&text).unwrap()).unwrap();
    definition.provenance.ast_sha256 =
        sha256(format!("[{}]", definition.provenance.abilities_json).as_bytes());
    definition.static_abilities = vec![granted_land_mana_static()];
    definition.mana_abilities = vec![granted_land_mana_printed()];
    definition.capabilities = definition.requirements();
    definition
}
