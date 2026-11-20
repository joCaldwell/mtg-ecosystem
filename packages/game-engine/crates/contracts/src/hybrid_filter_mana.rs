//! A colored hybrid input and a single choice of an entire two-mana result.
//! CR 106.7, 107.4e, 601.2b/h, 602.2b, 605.3b and 733.1 (2026-09-25).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

pub fn hybrid_filter_colors(definition: &Definition) -> Option<[Color; 2]> {
    match definition.mana_abilities.as_slice() {
        [
            _,
            ManaAbility {
                production: ManaProduction::HybridFilter { colors },
                ..
            },
        ] => Some(*colors),
        _ => None,
    }
}

pub fn hybrid_filter_source([first, second]: [Color; 2]) -> Option<(String, Value)> {
    if first == second || [first, second].contains(&Color::C) {
        return None;
    }
    let symbol = format!("{first:?}/{second:?}");
    let text = format!(
        "{{T}}: Add {{C}}.\n{{{symbol}}}, {{T}}: Add {{{first:?}}}{{{first:?}}}, {{{first:?}}}{{{second:?}}}, or {{{second:?}}}{{{second:?}}}."
    );
    let tap = json!({"cost":"tap-self"});
    let add = |mana| json!([{"steps":[{"effect":"add-mana","who":{"player":"you"},"mana":mana}]}]);
    let discounted = json!({"mana":"choice","options":[
        {"mana":"fixed","symbols":[first,first]},
        {"mana":"fixed","symbols":[first,second]},
        {"mana":"fixed","symbols":[second,second]}
    ]});
    let abilities = json!([
        {"kind":"activated","costs":[tap],"effects":add(json!({"mana":"fixed","symbols":["C"]}))},
        {"kind":"activated","costs":[{"cost":"mana","symbols":[symbol]},tap],"effects":add(discounted)}
    ]);
    Some((text, abilities))
}

pub fn hybrid_filter_abilities(colors: [Color; 2]) -> [ManaAbility; 2] {
    let tap = PermanentActivationCosts {
        tap: true,
        sacrifice: false,
        mana: None,
        remove_source_plus_one_counters: None,
    };
    [
        ManaAbility {
            self_damage: None,
            restriction: None,
            costs: tap.clone(),
            production: ManaProduction::Fixed {
                symbols: vec![Color::C],
            },
            source_block: 0,
        },
        ManaAbility {
            self_damage: None,
            restriction: None,
            costs: tap,
            production: ManaProduction::HybridFilter { colors },
            source_block: 1,
        },
    ]
}

pub(super) fn validate(d: &Definition) -> Result<bool, String> {
    if !d
        .mana_abilities
        .iter()
        .any(|a| matches!(a.production, ManaProduction::HybridFilter { .. }))
    {
        return Ok(false);
    }
    let bad = "requires the complete colorless tap and hybrid filter mana source";
    let colors = hybrid_filter_colors(d).ok_or(bad)?;
    let (text, ast) = hybrid_filter_source(colors).ok_or(bad)?;
    let source: Value = strict_read(&d.provenance.source_json)?;
    let produced: Vec<Color> =
        serde_json::from_value(source["produced_mana"].clone()).map_err(|_| bad)?;
    let set = |c: &[Color]| c.iter().copied().collect::<BTreeSet<_>>();
    let c = &d.characteristics;
    if d.mana_abilities != hybrid_filter_abilities(colors)
        || c.kind != CardKind::Land
        || c.basic
        || c.legendary
        || c.snow
        || !c.subtypes.is_empty()
        || !c.additional_types.is_empty()
        || !c.colors.is_empty()
        || c.color_identity.len() != 2
        || set(&c.color_identity) != set(&colors)
        || d.x_mana_cost
        || d.spell.is_some()
        || d.intrinsic_zone != Some(FunctioningZone::Battlefield)
        || !d.keywords.is_empty()
        || !d.activated.is_empty()
        || !d.triggered.is_empty()
        || !d.static_abilities.is_empty()
        || source["name"] != d.name
        || source["oracle_id"] != d.oracle_id
        || source["id"] != d.provenance.printing_id
        || source["oracle_text"] != text
        || source["type_line"] != "Land"
        || source["layout"] != "normal"
        || source["keywords"] != json!([])
        || source["colors"] != json!([])
        || source["mana_cost"] != ""
        || source["color_identity"] != json!(c.color_identity)
        || produced.len() != 3
        || set(&produced) != set(&[colors[0], colors[1], Color::C])
        || strict_read::<Value>(&d.provenance.abilities_json)? != ast
        || d.source_labels.as_ref().is_some_and(|labels| {
            labels.text != text
                || !labels.blocks.iter().map(String::as_str).eq(text.lines())
                || labels.mana != [0, 1]
                || !labels.statics.is_empty()
                || !labels.activated.is_empty()
                || !labels.triggered.is_empty()
                || !labels.spell_modes.is_empty()
        })
    {
        return Err(bad.into());
    }
    Ok(true)
}

pub(super) fn selector() -> Value {
    json!({"required":["mana_abilities"],"properties":{"mana_abilities":{"contains":{
        "required":["production"],"properties":{"production":{"required":["mana"],
            "properties":{"mana":{"const":"hybrid_filter"}}}}
    }}}})
}

pub(super) fn extend_schema(schema: &mut Value) {
    use Capability::*;
    let capabilities = [
        NormalLayoutV1,
        PermanentManaV1,
        LandPlayV1,
        HybridFilterManaV1,
    ];
    let tap = json!({"properties":{"tap":{"const":true},"sacrifice":{"const":false},
        "mana":{"type":"null"},"remove_source_plus_one_counters":{"type":"null"}}});
    let ability = |block, production| {
        json!({"properties":{"source_block":{"const":block},
        "costs":tap,"production":production,"self_damage":{"type":"null"},"restriction":{"type":"null"}}})
    };
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "if":selector(),
        "then":{"required":["intrinsic_zone"],"properties":{
            "capabilities":{"items":{"enum":capabilities},
                "allOf":capabilities.map(|cap|json!({"contains":{"const":cap}}))},
            "intrinsic_zone":{"const":"battlefield"},"spell":{"type":"null"},
            "characteristics":{"properties":{"kind":{"const":"land"},
                "basic":{"const":false},"legendary":{"const":false},"snow":{"const":false},
                "subtypes":{"maxItems":0},"additional_types":{"maxItems":0},
                "colors":{"maxItems":0},"color_identity":{"minItems":2,"maxItems":2}}},
            "x_mana_cost":{"const":false},"keywords":{"maxItems":0},"activated":{"maxItems":0},
            "triggered":{"maxItems":0},"static_abilities":{"maxItems":0},
            "mana_abilities":{"minItems":2,"items":false,"prefixItems":[
                ability(0,json!({"const":{"mana":"fixed","symbols":["C"]}})),
                ability(1,json!({"properties":{"mana":{"const":"hybrid_filter"},"colors":{
                    "minItems":2,"maxItems":2,"uniqueItems":true,"items":{"enum":["W","U","B","R","G"]},
                    "prefixItems":[{"enum":["W","U","B","R","G"]},{"enum":["W","U","B","R","G"]}]
                }}}))
            ]}
        }},
        "else":{"properties":{"capabilities":{"not":{"contains":{"const":HybridFilterManaV1}}}}}
    }));
}

#[cfg(test)]
pub(super) fn witness() -> Definition {
    let mut d = pain_mana::witness();
    let record: Value = strict_read(include_str!(
        "../../../tests/fixtures/f1603384-4361-49c9-98aa-7785fc3504c4.json"
    ))
    .unwrap();
    let manifest = include_str!("../../../tests/fixtures/archive-manifest-current.json").trim_end();
    let header: Value = strict_read(manifest).unwrap();
    let source = &record["source"];
    d.name = source["name"].as_str().unwrap().into();
    d.oracle_id = source["oracle_id"].as_str().unwrap().into();
    d.provenance.printing_id = source["id"].as_str().unwrap().into();
    d.provenance.source_index = record["sourceIndex"].as_u64().unwrap().try_into().unwrap();
    d.provenance.source_json = canonical(source).unwrap();
    d.provenance.source_sha256 = sha256(d.provenance.source_json.as_bytes());
    d.provenance.abilities_json = canonical(&record["faces"][0]["result"]["abilities"]).unwrap();
    d.provenance.ast_sha256 = sha256(format!("[{}]", d.provenance.abilities_json).as_bytes());
    d.provenance.binding_sha256 = record["bindingSha256"].as_str().unwrap().into();
    d.provenance.archive_manifest_sha256 = sha256(manifest.as_bytes());
    d.provenance.parser_sha256 = header["parser"]["sourceSha256"].as_str().unwrap().into();
    d.characteristics.color_identity =
        serde_json::from_value(source["color_identity"].clone()).unwrap();
    d.mana_abilities = hybrid_filter_abilities([Color::U, Color::R]).to_vec();
    d.capabilities = d.requirements();
    d
}

#[test]
fn hybrid_filter_mana_requires_both_exact_branches() {
    let mut definition = witness();
    definition.validate().unwrap();
    definition.mana_abilities[1].production = ManaProduction::HybridFilter {
        colors: [Color::U, Color::G],
    };
    assert!(definition.validate().is_err());
}
