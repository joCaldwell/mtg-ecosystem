//! CR 109.5, 201.5 and 614.12 (2026-09-25). The entering controller's
//! common-prior lands determine whether this source enters tapped.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const INVALID: &str = "requires exact other-land count entry and one tap-only dual mana ability";
const PREFIX: &str = "This land enters tapped unless you control ";

pub(super) fn validate(d: &Definition) -> Result<(), String> {
    if !d
        .capabilities
        .contains(&Capability::ConditionalOtherLandEntryV1)
    {
        return Ok(());
    }
    let [
        StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect:
                StaticEffect::EntersTappedUnlessControllerOtherLands {
                    comparison,
                    threshold,
                },
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
    let c = &d.characteristics;
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
    {
        return Err(INVALID.into());
    }

    let threshold = threshold.to_string().parse::<i64>().map_err(|_| INVALID)?;
    let (comparison_op, printed_suffix) = match comparison {
        LandCountComparison::AtLeast => ("ge", " or more other lands."),
        LandCountComparison::AtMost => ("le", " or fewer other lands."),
    };
    let source: Value = strict_read(&d.provenance.source_json)?;
    let text = source["oracle_text"].as_str().ok_or(INVALID)?;
    let Some((entry, mana)) = text.split_once('\n') else {
        return Err(INVALID.into());
    };
    let printed_amount = entry
        .strip_prefix(PREFIX)
        .and_then(|entry| entry.strip_suffix(printed_suffix))
        .ok_or(INVALID)?;
    let set = |colors: &[Color]| colors.iter().copied().collect::<BTreeSet<_>>();
    let produced: Vec<Color> =
        serde_json::from_value(source["produced_mana"].clone()).map_err(|_| INVALID)?;
    if threshold < 0
        || !matches_fixed_amount(printed_amount, threshold)
        || mana != format!("{{T}}: Add {{{first:?}}} or {{{second:?}}}.")
        || source["type_line"] != "Land"
        || source["layout"] != "normal"
        || source["name"] != d.name
        || source["oracle_id"] != d.oracle_id
        || source["id"] != d.provenance.printing_id
        || source["mana_cost"] != ""
        || source["keywords"] != json!([])
        || source["colors"] != json!([])
        || source["color_identity"] != json!(c.color_identity)
        || c.color_identity.len() != 2
        || set(&c.color_identity) != set(colors)
        || produced.len() != 2
        || set(&produced) != set(colors)
    {
        return Err(INVALID.into());
    }

    let expected = json!([
        {"kind":"static","effect":{"static":"enters-tapped","what":{"ref":"this","noun":"land"},
            "conditionKind":"unless","condition":{"condition":"control-count","who":{"player":"you"},
            "of":{"other":true,"types":["land"]},"comparison":{"op":comparison_op,"value":{
                "amount":"fixed","value":threshold
            }}
        }}},
        {"kind":"activated","costs":[{"cost":"tap-self"}],"effects":[{"steps":[{
            "effect":"add-mana","who":{"player":"you"},"mana":{"mana":"choice","options":[
                {"mana":"fixed","symbols":[first]},{"mana":"fixed","symbols":[second]}
            ]}
        }]}]}
    ]);
    if strict_read::<Value>(&d.provenance.abilities_json)? != expected
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

pub(super) fn extend_schema(schema: &mut Value) {
    use Capability::*;

    let member = "enters_tapped_unless_controller_other_lands";
    let capability = json!({"contains":{"const":ConditionalOtherLandEntryV1}});
    let effect = json!({"type":"object","required":[member]});
    let required = [
        NormalLayoutV1,
        LandPlayV1,
        PermanentManaV1,
        ListedManaChoiceV1,
        ConditionalOtherLandEntryV1,
    ];
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "if":{"required":["static_abilities"],"properties":{
            "static_abilities":{"contains":{"required":["effect"],"properties":{"effect":effect}}}
        }},
        "then":{"required":["intrinsic_zone","mana_abilities"],"properties":{
            "capabilities":{"items":{"enum":required},"allOf":required.map(|cap|json!({"contains":{"const":cap}}))},
            "characteristics":{"properties":{
                "kind":{"const":"land"},"basic":{"const":false},"legendary":{"const":false},"snow":{"const":false},
                "additional_types":{"maxItems":0},"subtypes":{"maxItems":0},"colors":{"maxItems":0}
            }},
            "x_mana_cost":{"const":false},"intrinsic_zone":{"const":"battlefield"},"spell":{"type":"null"},
            "activated":{"maxItems":0},"triggered":{"maxItems":0},"keywords":{"maxItems":0},
            "static_abilities":{"minItems":1,"maxItems":1,"items":{"properties":{
                "functioning_zone":{"const":"battlefield"},"effect":effect
            }}},
            "mana_abilities":{"minItems":1,"maxItems":1,"items":{"properties":{
                "source_block":{"const":1},
                "production":{"properties":{"mana":{"const":"choice"},"colors":{"minItems":2,"maxItems":2}}}
            }}},
            "source_labels":{"required":["statics"],"properties":{
                "blocks":{"minItems":2,"maxItems":2},"mana":{"const":[1]},"statics":{"const":[0]},
                "triggered":{"maxItems":0},"activated":{"maxItems":0},"spell_modes":{"maxItems":0}
            }}
        }},
        "else":{"properties":{"capabilities":{"not":capability}}}
    }));
}

#[cfg(test)]
pub(super) fn witness() -> Definition {
    let archive = include_str!("../../../tests/fixtures/eb0d8093-5f93-4b25-9384-08f9731bfb28.json");
    let record: Value = strict_read(archive).unwrap();
    let source = &record["source"];
    let mut d = listed_mana_choice::witness();
    d.name = source["name"].as_str().unwrap().into();
    d.oracle_id = source["oracle_id"].as_str().unwrap().into();
    d.provenance.printing_id = source["id"].as_str().unwrap().into();
    d.provenance.source_json = canonical(source).unwrap();
    d.provenance.source_sha256 = sha256(d.provenance.source_json.as_bytes());
    d.provenance.abilities_json = canonical(&record["faces"][0]["result"]["abilities"]).unwrap();
    d.provenance.ast_sha256 = sha256(format!("[{}]", d.provenance.abilities_json).as_bytes());
    d.provenance.binding_sha256 = record["bindingSha256"].as_str().unwrap().into();
    d.characteristics.color_identity = vec![Color::R, Color::U];
    d.mana_abilities[0].source_block = 1;
    d.mana_abilities[0].production = ManaProduction::Choice {
        colors: vec![Color::U, Color::R],
    };
    d.static_abilities = vec![StaticAbility {
        functioning_zone: FunctioningZone::Battlefield,
        effect: StaticEffect::EntersTappedUnlessControllerOtherLands {
            comparison: LandCountComparison::AtMost,
            threshold: 2.into(),
        },
    }];
    d.capabilities = d.requirements();
    d
}
