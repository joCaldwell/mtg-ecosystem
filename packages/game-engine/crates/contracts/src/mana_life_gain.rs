//! A tap-only artifact mana ability with an immediate life-gain effect.
//! CR 109.5, 119.3, 119.7, 605.1a and 605.3b (2026-09-25).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const PREFIX: &str = "{T}: Add ";
const LIFE: &str = ". You gain ";
const INVALID: &str = "requires complete fixed tap mana followed by mandatory controller life gain";

pub fn mana_life_gain_source(text: &str) -> Option<(ManaAbility, Value)> {
    let (printed, quantity) = text.strip_prefix(PREFIX)?.split_once(LIFE)?;
    let printed_amount = quantity.strip_suffix(" life.")?;
    let amount = printed_amount.parse::<i64>().ok()?;
    if amount <= 0 || amount.to_string() != printed_amount || printed.is_empty() {
        return None;
    }
    let symbols = mana_spend::parse_fixed_mana_symbols(printed)?;
    let you = json!({"player":"you"});
    let ast = json!([{"kind":"activated","costs":[{"cost":"tap-self"}],"effects":[
        {"steps":[{"effect":"add-mana","who":you,"mana":{"mana":"fixed","symbols":symbols}}]},
        {"steps":[{"effect":"gain-life","who":you,"amount":{"amount":"fixed","value":amount}}]}
    ]}]);
    Some((
        ManaAbility {
            self_damage: None,
            restriction: None,
            costs: PermanentActivationCosts {
                tap: true,
                mana: None,
                sacrifice: false,
                remove_source_plus_one_counters: None,
            },
            production: ManaProduction::FixedWithLifeGain {
                symbols,
                life: amount.into(),
            },
            source_block: 0,
        },
        ast,
    ))
}

pub(super) fn present(d: &Definition) -> bool {
    d.mana_abilities
        .iter()
        .any(|ability| matches!(ability.production, ManaProduction::FixedWithLifeGain { .. }))
}

fn permanent_spell() -> SpellProgram {
    SpellProgram {
        functioning_zone: FunctioningZone::Stack,
        timing: SpellTiming::MainPhase,
        resolution: SpellResolution::Permanent,
        targets: vec![],
        instructions: vec![],
        modes: vec![],
        optional_additional_costs: vec![],
        additional_sacrifice: None,
    }
}

pub(super) fn validate(d: &Definition) -> Result<(), String> {
    let raw = &d.provenance.source_json;
    if !present(d)
        && !d.capabilities.contains(&Capability::ManaLifeGainV1)
        && !(raw.contains(PREFIX) && raw.contains(LIFE))
        && !raw.contains(r"\u")
    {
        return Ok(());
    }
    let source: Value = strict_read(raw)?;
    let Some(text) = source["oracle_text"].as_str() else {
        return Err(INVALID.into());
    };
    let Some((ability, ast)) = mana_life_gain_source(text) else {
        return if present(d) || d.capabilities.contains(&Capability::ManaLifeGainV1) {
            Err(INVALID.into())
        } else {
            Ok(())
        };
    };
    let c = &d.characteristics;
    let ManaProduction::FixedWithLifeGain { symbols, .. } = &ability.production else {
        return Err(INVALID.into());
    };
    let produced: Vec<Color> =
        serde_json::from_value(source["produced_mana"].clone()).map_err(|_| INVALID)?;
    let colors = |values: &[Color]| values.iter().copied().collect::<BTreeSet<_>>();
    if c.kind != CardKind::Artifact
        || c.legendary
        || c.basic
        || c.snow
        || !c.additional_types.is_empty()
        || !c.subtypes.is_empty()
        || !c.hybrid_mana.is_empty()
        || d.x_mana_cost
        || d.intrinsic_zone.is_some()
        || d.spell.as_ref() != Some(&permanent_spell())
        || !d.activated.is_empty()
        || !d.triggered.is_empty()
        || !d.static_abilities.is_empty()
        || !d.keywords.is_empty()
        || d.mana_abilities.as_slice() != [ability.clone()]
        || source["layout"] != "normal"
        || source["type_line"] != "Artifact"
        || source["name"] != d.name
        || source["oracle_id"] != d.oracle_id
        || source["id"] != d.provenance.printing_id
        || source["keywords"] != json!([])
        || source["colors"] != json!(c.colors)
        || source["color_identity"] != json!(c.color_identity)
        || source["mana_cost"] != c.mana_cost.printed_fixed_cost()
        || colors(&produced) != colors(symbols)
        || strict_read::<Value>(&d.provenance.abilities_json)? != ast
        || d.source_labels.as_ref().is_some_and(|labels| {
            labels.text != text
                || labels.blocks != [text]
                || labels.mana != [0]
                || !labels.activated.is_empty()
                || !labels.triggered.is_empty()
                || !labels.statics.is_empty()
                || !labels.spell_modes.is_empty()
        })
    {
        return Err(INVALID.into());
    }
    Ok(())
}

pub(super) fn extend_schema(schema: &mut Value) {
    use Capability::*;
    let caps = [
        NormalLayoutV1,
        CastArtifactV1,
        PermanentManaV1,
        ManaLifeGainV1,
        LifeGainV1,
    ];
    let cap = json!({"contains":{"const":ManaLifeGainV1}});
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "if":{"anyOf":[
            {"required":["mana_abilities"],"properties":{"mana_abilities":{"contains":{"properties":{
                "production":{"properties":{"mana":{"const":"fixed_with_life_gain"}}}
            }}}}},
            {"properties":{"provenance":{"properties":{"source_json":{"pattern":r#""oracle_text"\s*:\s*"\{T\}: Add (?:\{[WUBRGC]\})+\. You gain [1-9][0-9]* life\.""#}}}}}
        ]},
        "then":{"required":["spell","mana_abilities"],"properties":{
            "spell":{"const":permanent_spell()},
            "capabilities":{"items":{"enum":caps},"allOf":caps.map(|c|json!({"contains":{"const":c}}))},
            "characteristics":{"properties":{"kind":{"const":"artifact"},"legendary":{"const":false},
                "basic":{"const":false},"snow":{"const":false},"hybrid_mana":{"maxItems":0},
                "additional_types":{"maxItems":0},"subtypes":{"maxItems":0}}},
            "x_mana_cost":{"const":false},"intrinsic_zone":{"type":"null"},
            "activated":{"maxItems":0},"triggered":{"maxItems":0},"static_abilities":{"maxItems":0},"keywords":{"maxItems":0},
            "mana_abilities":{"minItems":1,"maxItems":1,"items":{"properties":{
                "source_block":{"const":0},"self_damage":{"type":"null"},"restriction":{"type":"null"},
                "costs":{"const":PermanentActivationCosts {tap:true,mana:None,sacrifice:false,remove_source_plus_one_counters:None}},
                "production":{"properties":{"mana":{"const":"fixed_with_life_gain"},
                    "symbols":{"minItems":1},"life":{"pattern":"^[1-9][0-9]*$","maxLength":19}}}
            }}}
        }},
        "else":{"properties":{"capabilities":{"not":cap}}}
    }));
}

#[cfg(test)]
pub(super) fn witness() -> Definition {
    let raw: Value = strict_read(include_str!(
        "../../../tests/fixtures/1b3d7fce-e9fe-4176-9d9a-472415826cdd.json"
    ))
    .unwrap();
    let catalog = Catalog::read(include_str!(
        "../../../tests/fixtures/expected-mana-ir.json"
    ))
    .unwrap();
    let mut d = catalog
        .definitions
        .into_iter()
        .find(|d| d.name == "Sol Ring")
        .unwrap();
    let source = &raw["source"];
    d.name = source["name"].as_str().unwrap().into();
    d.oracle_id = source["oracle_id"].as_str().unwrap().into();
    d.provenance.printing_id = source["id"].as_str().unwrap().into();
    d.provenance.source_index = raw["sourceIndex"].as_u64().unwrap().try_into().unwrap();
    d.provenance.source_json = canonical(source).unwrap();
    d.provenance.source_sha256 = sha256(d.provenance.source_json.as_bytes());
    d.provenance.abilities_json = canonical(&raw["faces"][0]["result"]["abilities"]).unwrap();
    d.provenance.ast_sha256 = sha256(format!("[{}]", d.provenance.abilities_json).as_bytes());
    d.provenance.binding_sha256 = raw["bindingSha256"].as_str().unwrap().into();
    let archive = accepted_manifest(include_str!(
        "../../../tests/fixtures/archive-manifest-current.json"
    ))
    .unwrap();
    d.provenance.archive_manifest_sha256 = archive.manifest_sha256.clone();
    d.provenance.parser_sha256 = archive.parser_source_sha256.clone();
    let text = source["oracle_text"].as_str().unwrap();
    d.mana_abilities = vec![mana_life_gain_source(text).unwrap().0];
    d.characteristics.mana_cost.generic = 3.into();
    d.source_labels = None;
    d.capabilities = d.requirements();
    d
}

#[test]
fn mana_life_gain_contract_rejects_omitted_rider() {
    let mut d = witness();
    d.validate().unwrap();
    d.mana_abilities[0].production = ManaProduction::Fixed {
        symbols: vec![Color::C],
    };
    d.capabilities = d.requirements();
    assert!(d.validate().is_err());
}
