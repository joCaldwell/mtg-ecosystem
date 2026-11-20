//! CR 109.5, 205.3i, 305.6 and 614.12 (2026-09-25): a self-entry
//! replacement checks the entering controller's current basic land types.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const INVALID: &str = "requires complete checkland entry and one tap-only dual mana ability";

pub(super) fn validate(d: &Definition) -> Result<(), String> {
    if !d
        .capabilities
        .contains(&Capability::ConditionalLandTypeEntryV1)
    {
        return Ok(());
    }
    let [
        StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect: StaticEffect::EntersTappedUnlessControllerLandTypes { subtypes },
        },
    ] = d.static_abilities.as_slice()
    else {
        return Err(INVALID.into());
    };
    let [
        ManaAbility {
            production: ManaProduction::Choice { colors },
            source_block: 1,
            ..
        },
    ] = d.mana_abilities.as_slice()
    else {
        return Err(INVALID.into());
    };
    listed_mana_choice::validate(d)?;

    let [first, second] = colors.as_slice() else {
        return Err(INVALID.into());
    };
    let [left, right] = subtypes.map(BasicLandType::subtype);
    let type_colors = subtypes.map(BasicLandType::color);
    let c = &d.characteristics;
    let set = |colors: &[Color]| colors.iter().copied().collect::<BTreeSet<_>>();
    if c.kind != CardKind::Land
        || c.basic
        || c.legendary
        || c.snow
        || !c.additional_types.is_empty()
        || !c.subtypes.is_empty()
        || !c.colors.is_empty()
        || !d.keywords.is_empty()
        || !d.activated.is_empty()
        || !d.triggered.is_empty()
        || d.x_mana_cost
        || c.color_identity.len() != 2
        || set(&c.color_identity) != set(colors)
        || set(&type_colors) != set(colors)
    {
        return Err(INVALID.into());
    }

    let entry = format!(
        "This land enters tapped unless you control {} or {}.",
        with_article(left),
        with_article(right)
    );
    let mana = format!("{{T}}: Add {{{first:?}}} or {{{second:?}}}.");
    let text = format!("{entry}\n{mana}");
    let source: Value = strict_read(&d.provenance.source_json)?;
    let produced: Vec<Color> =
        serde_json::from_value(source["produced_mana"].clone()).map_err(|_| INVALID)?;
    let ast = json!([
        {"kind":"static","effect":{"static":"enters-tapped","what":{"ref":"this","noun":"land"},
            "conditionKind":"unless","condition":{"condition":"control","who":{"player":"you"},
                "what":{"ref":"filter","filter":{"anyOf":[{"subtypes":[left]},{"subtypes":[right]}]},
                    "count":{"amount":"fixed","value":1}}
            }}},
        {"kind":"activated","costs":[{"cost":"tap-self"}],"effects":[{"steps":[{
            "effect":"add-mana","who":{"player":"you"},"mana":{"mana":"choice","options":[
                {"mana":"fixed","symbols":[first]},{"mana":"fixed","symbols":[second]}
            ]}
        }]}]}
    ]);
    if source["oracle_text"] != text
        || source["type_line"] != "Land"
        || source["layout"] != "normal"
        || source["name"] != d.name
        || source["oracle_id"] != d.oracle_id
        || source["id"] != d.provenance.printing_id
        || source["mana_cost"] != ""
        || source["keywords"] != json!([])
        || source["colors"] != json!([])
        || source["color_identity"] != json!(c.color_identity)
        || produced.len() != 2
        || set(&produced) != set(colors)
        || strict_read::<Value>(&d.provenance.abilities_json)? != ast
        || d.source_labels.as_ref().is_some_and(|labels| {
            labels.text != text
                || labels.blocks != [entry, mana]
                || labels.statics != [0]
                || labels.mana != [1]
                || !labels.triggered.is_empty()
                || !labels.activated.is_empty()
                || !labels.spell_modes.is_empty()
        })
    {
        return Err(INVALID.into());
    }
    Ok(())
}

fn with_article(subtype: &str) -> String {
    let article = if subtype == "Island" { "an" } else { "a" };
    format!("{article} {subtype}")
}

pub(super) fn extend_schema(schema: &mut Value) {
    use Capability::*;

    let member = "enters_tapped_unless_controller_land_types";
    let effect = json!({"type":"object","required":[member]});
    let subtypes = json!({"properties":{(member):{"properties":{"subtypes":{
        "minItems":2,"maxItems":2,"uniqueItems":true
    }}}}});
    let required = [
        NormalLayoutV1,
        LandPlayV1,
        PermanentManaV1,
        ListedManaChoiceV1,
        ConditionalLandTypeEntryV1,
    ];
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "if":{"required":["static_abilities"],"properties":{"static_abilities":{
            "contains":{"required":["effect"],"properties":{"effect":effect}}
        }}},
        "then":{"required":["intrinsic_zone","mana_abilities"],"properties":{
            "capabilities":{"items":{"enum":required},"allOf":required.map(|cap|json!({"contains":{"const":cap}}))},
            "characteristics":{"properties":{"kind":{"const":"land"},"basic":{"const":false},
                "legendary":{"const":false},"snow":{"const":false},"additional_types":{"maxItems":0},
                "subtypes":{"maxItems":0},"colors":{"maxItems":0},"color_identity":{"minItems":2,"maxItems":2}
            }},
            "x_mana_cost":{"const":false},"intrinsic_zone":{"const":"battlefield"},"spell":{"type":"null"},
            "activated":{"maxItems":0},"triggered":{"maxItems":0},"keywords":{"maxItems":0},
            "static_abilities":{"minItems":1,"maxItems":1,"items":{"properties":{
                "functioning_zone":{"const":"battlefield"},"effect":{"allOf":[effect,subtypes]}
            }}},
            "mana_abilities":{"minItems":1,"maxItems":1,"items":{"properties":{
                "source_block":{"const":1},
                "production":{"properties":{"mana":{"const":"choice"},"colors":{"minItems":2,"maxItems":2}}}
            }}},
            "source_labels":{"required":["statics"],"properties":{"blocks":{"minItems":2,"maxItems":2},
                "mana":{"const":[1]},"statics":{"const":[0]},"triggered":{"maxItems":0},
                "activated":{"maxItems":0},"spell_modes":{"maxItems":0}
            }}
        }},
        "else":{"properties":{"capabilities":{"not":{"contains":{"const":ConditionalLandTypeEntryV1}}}}}
    }));
}
