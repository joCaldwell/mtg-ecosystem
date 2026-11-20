//! CR 106.1, 107.3c, 302.6, 605.1a, 605.3b and 608.2h (2026-09-25).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

pub(super) fn present(d: &Definition) -> bool {
    d.mana_abilities
        .iter()
        .any(|a| matches!(a.production, ManaProduction::AnyOneColor { .. }))
}

pub(super) fn validate(d: &Definition) -> Result<(), String> {
    if !present(d) {
        return Ok(());
    }
    let invalid = "requires complete tap-only same-color mana production";
    let [
        ManaAbility {
            self_damage: None,
            restriction: None,
            costs:
                PermanentActivationCosts {
                    tap: true,
                    sacrifice: false,
                    mana: None,
                    remove_source_plus_one_counters: None,
                },
            production: ManaProduction::AnyOneColor { amount },
            source_block: 0,
        },
    ] = d.mana_abilities.as_slice()
    else {
        return Err(invalid.into());
    };

    let c = &d.characteristics;
    let valid = match amount {
        SingleColorManaAmount::Fixed { quantity } => {
            *quantity > Quantity::zero()
                && c.kind == CardKind::Artifact
                && c.additional_types.is_empty()
        }
        SingleColorManaAmount::ControllerEnchantments => {
            c.kind == CardKind::Creature && c.additional_types == [CardKind::Enchantment]
        }
    };
    if !valid
        || c.legendary
        || c.basic
        || c.snow
        || d.x_mana_cost
        || !d.activated.is_empty()
        || !d.triggered.is_empty()
        || !d.static_abilities.is_empty()
        || !d.keywords.is_empty()
    {
        return Err(invalid.into());
    }
    let source: Value = strict_read(&d.provenance.source_json)?;
    let text = source["oracle_text"].as_str().ok_or(invalid)?;
    let mut effects = json!([{"steps":[{"effect":"add-mana","who":{"player":"you"},
        "mana":{"mana":"any-one-color"}}]}]);
    let mana = match amount {
        SingleColorManaAmount::Fixed { quantity } => {
            let value = quantity.to_string().parse::<i64>().map_err(|_| invalid)?;
            let printed = text
                .strip_prefix("{T}: Add ")
                .and_then(|text| text.strip_suffix(" mana of any one color."))
                .ok_or(invalid)?;
            if !matches_fixed_amount(printed, value) {
                return Err(invalid.into());
            }
            json!({"amount":"fixed","value":value})
        }
        SingleColorManaAmount::ControllerEnchantments => {
            if text
                != "{T}: Add X mana of any one color, where X is the number of enchantments you control."
            {
                return Err(invalid.into());
            }
            effects[0]["bindings"] = json!([{"variable":"x","value":{"amount":"count",
                "of":{"types":["enchantment"],"control":"you"}}}]);
            json!({"amount":"x"})
        }
    };
    effects[0]["steps"][0]["mana"]["amount"] = mana;
    if strict_read::<Value>(&d.provenance.abilities_json)?
        != json!([{
            "kind":"activated","costs":[{"cost":"tap-self"}],"effects":effects
        }])
    {
        return Err(invalid.into());
    }
    Ok(())
}

pub(super) fn extend_schema(schema: &mut Value) {
    use Capability::*;
    let selected = json!({"required":["production"],"properties":{
        "production":{"required":["mana"],"properties":{"mana":{"const":"any_one_color"}}}
    }});
    let common = [NormalLayoutV1, PermanentManaV1, SameColorManaV1];
    let branch = |kind, amount, additional_types, caps: &[Capability]| {
        let required = [common.as_slice(), caps].concat();
        json!({"properties":{
            "characteristics":{"properties":{"kind":{"const":kind},"additional_types":{"const":additional_types}}},
            "capabilities":{"items":{"enum":required},
                "allOf":required.iter().map(|cap| json!({"contains":{"const":cap}})).collect::<Vec<_>>()},
            "mana_abilities":{"items":{"properties":{"production":{"properties":{"amount":amount}}}}}
        }})
    };
    let fixed = branch(
        CardKind::Artifact,
        json!({"properties":{"kind":{"const":"fixed"},"quantity":{"pattern":"^[1-9][0-9]*$"}}}),
        json!([]),
        &[CastArtifactV1],
    );
    let counted = branch(
        CardKind::Creature,
        json!({"properties":{"kind":{"const":"controller_enchantments"}}}),
        json!([CardKind::Enchantment]),
        &[BaseCreatureV1, CastCreatureV1, CastEnchantmentV1],
    );
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "if":{"required":["mana_abilities"],"properties":{"mana_abilities":{"contains":selected}}},
        "then":{"oneOf":[fixed,counted],"properties":{
            "characteristics":{"properties":{"legendary":{"const":false},"basic":{"const":false},"snow":{"const":false}}},
            "x_mana_cost":{"const":false},"activated":{"maxItems":0},"triggered":{"maxItems":0},
            "static_abilities":{"maxItems":0},"keywords":{"maxItems":0},
            "mana_abilities":{"minItems":1,"maxItems":1,"items":{"properties":{
                "costs":{"properties":{"tap":{"const":true},"sacrifice":{"const":false},
                    "mana":{"type":"null"},"remove_source_plus_one_counters":{"type":"null"}}},
                "restriction":{"type":"null"},"self_damage":{"type":"null"},"source_block":{"const":0}
            }}}
        }},
        "else":{"properties":{"capabilities":{"not":{"contains":{"const":SameColorManaV1}}}}}
    }));
}
