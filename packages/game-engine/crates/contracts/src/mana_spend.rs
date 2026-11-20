//! Restricted mana remains restricted until spent or lost (CR 106.6).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ManaSpendRestriction {
    ActivatedAbility,
    ColorlessSpellOrActivatedAbility,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RestrictedMana {
    pub restriction: ManaSpendRestriction,
    pub color: Color,
    pub amount: Quantity,
}

impl RestrictedMana {
    pub fn key(&self) -> (ManaSpendRestriction, Color) {
        (self.restriction, self.color)
    }
}

use serde_json::{Value, json};

const PREFIX: &str = "{T}: Add ";
const SPEND: &str = ". Spend this mana only to ";
const INVALID: &str =
    "requires complete fixed tap mana and exact activation or colorless spell spend restriction";

pub(super) fn parse_fixed_mana_symbols(printed: &str) -> Option<Vec<Color>> {
    let mut symbols = vec![];
    let mut rest = printed;
    while !rest.is_empty() {
        let (symbol, tail) = rest.strip_prefix('{')?.split_once('}')?;
        symbols.push(serde_json::from_value(json!(symbol)).ok()?);
        rest = tail;
    }
    (!symbols.is_empty()).then_some(symbols)
}

pub fn restricted_mana_source(text: &str) -> Option<(ManaAbility, Value)> {
    let (printed, restriction) = text.strip_prefix(PREFIX)?.split_once(SPEND)?;
    let (spend, uses) = match restriction {
        "activate abilities." => (
            ManaSpendRestriction::ActivatedAbility,
            json!([{"use":"activate"}]),
        ),
        "cast a colorless spell or to activate an ability." => (
            ManaSpendRestriction::ColorlessSpellOrActivatedAbility,
            json!([{"use":"cast","filter":{"colorless":true,"cls":"spell"}},{"use":"activate"}]),
        ),
        _ => return None,
    };
    let symbols = parse_fixed_mana_symbols(printed)?;
    let ast = json!([{"kind":"activated","costs":[{"cost":"tap-self"}],"effects":[{"steps":[
        {"effect":"add-mana","who":{"player":"you"},"mana":{"mana":"fixed","symbols":symbols},
         "restriction":{"restriction":"only","uses":uses}}
    ]}]}]);
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
            production: ManaProduction::RestrictedFixed { symbols, spend },
            source_block: 0,
        },
        ast,
    ))
}

pub(super) fn present(d: &Definition) -> bool {
    d.mana_abilities
        .iter()
        .any(|a| matches!(a.production, ManaProduction::RestrictedFixed { .. }))
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
        && !d.capabilities.contains(&Capability::RestrictedManaV1)
        && !(raw.contains(PREFIX) && raw.contains(SPEND))
        && !raw.contains(r"\u")
    {
        return Ok(());
    }
    let source: Value = strict_read(raw)?;
    let Some(text) = source["oracle_text"].as_str() else {
        return Err(INVALID.into());
    };
    let Some((ability, ast)) = restricted_mana_source(text) else {
        return if present(d) || d.capabilities.contains(&Capability::RestrictedManaV1) {
            Err(INVALID.into())
        } else {
            Ok(())
        };
    };
    let c = &d.characteristics;
    let ManaProduction::RestrictedFixed { symbols, .. } = &ability.production else {
        return Err(INVALID.into());
    };
    let produced: Vec<Color> =
        serde_json::from_value(source["produced_mana"].clone()).map_err(|_| INVALID)?;
    let colors = |values: &[Color]| values.iter().copied().collect::<BTreeSet<_>>();
    if c.kind != CardKind::Creature
        || c.legendary
        || c.basic
        || c.snow
        || !c.additional_types.is_empty()
        || c.subtypes.is_empty()
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
        || source["type_line"] != format!("Creature — {}", c.subtypes.join(" "))
        || source["name"] != d.name
        || source["oracle_id"] != d.oracle_id
        || source["id"] != d.provenance.printing_id
        || source["keywords"] != json!([])
        || source["colors"] != json!(c.colors)
        || source["color_identity"] != json!(c.color_identity)
        || source["power"] != json!(c.power.as_ref().map(ToString::to_string))
        || source["toughness"] != json!(c.toughness.as_ref().map(ToString::to_string))
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
        BaseCreatureV1,
        CastCreatureV1,
        PermanentManaV1,
        RestrictedManaV1,
    ];
    let cap = json!({"contains":{"const":RestrictedManaV1}});
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "if":{"anyOf":[
            {"required":["mana_abilities"],"properties":{"mana_abilities":{"contains":{"properties":{
                "production":{"properties":{"mana":{"const":"restricted_fixed"}}}
            }}}}},
            {"properties":{"provenance":{"properties":{"source_json":{"pattern":r#""oracle_text"\s*:\s*"\{T\}: Add (?:\{[WUBRGC]\})+\. Spend this mana only to (?:activate abilities|cast a colorless spell or to activate an ability)\.""#}}}}}
        ]},
        "then":{"required":["spell","mana_abilities"],"properties":{
            "spell":{"const":permanent_spell()},
            "capabilities":{"items":{"enum":caps},"allOf":caps.map(|c|json!({"contains":{"const":c}}))},
            "characteristics":{"properties":{"kind":{"const":"creature"},"legendary":{"const":false},
                "basic":{"const":false},"snow":{"const":false},"hybrid_mana":{"maxItems":0},
                "additional_types":{"maxItems":0},"subtypes":{"minItems":1}}},
            "x_mana_cost":{"const":false},"intrinsic_zone":{"type":"null"},
            "activated":{"maxItems":0},"triggered":{"maxItems":0},"static_abilities":{"maxItems":0},"keywords":{"maxItems":0},
            "mana_abilities":{"minItems":1,"maxItems":1,"items":{"properties":{
                "source_block":{"const":0},"self_damage":{"type":"null"},"restriction":{"type":"null"},
                "costs":{"const":PermanentActivationCosts {tap:true,mana:None,sacrifice:false,remove_source_plus_one_counters:None}},
                "production":{"properties":{"mana":{"const":"restricted_fixed"},"symbols":{"minItems":1}}}
            }}}
        }},
        "else":{"properties":{"capabilities":{"not":cap}}}
    }));
}

#[cfg(test)]
pub(super) fn witness() -> Definition {
    let raw: Value = strict_read(include_str!(
        "../../../tests/fixtures/215fd0b4-5817-4335-a0ed-1694e44bde41.json"
    ))
    .unwrap();
    let catalog = Catalog::read(include_str!(
        "../../../tests/fixtures/expected-mana-ir.json"
    ))
    .unwrap();
    let mut d = catalog
        .definitions
        .into_iter()
        .find(|d| d.name == "Grizzly Bears")
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
    d.mana_abilities = vec![
        restricted_mana_source(source["oracle_text"].as_str().unwrap())
            .unwrap()
            .0,
    ];
    d.characteristics.mana_cost = ManaCost {
        generic: 0.into(),
        colored: vec![Color::U],
    };
    d.characteristics.subtypes = vec!["Octopus".into(), "Advisor".into()];
    d.characteristics.colors = vec![Color::U];
    d.characteristics.color_identity = vec![Color::U];
    d.characteristics.power = Some(1.into());
    d.characteristics.toughness = Some(1.into());
    d.source_labels = None;
    d.capabilities = d.requirements();
    d
}

#[test]
fn restricted_mana_rejects_unrestricted_reinterpretation() {
    let mut d = witness();
    d.validate().unwrap();
    d.mana_abilities[0].production = ManaProduction::Fixed {
        symbols: vec![Color::C, Color::U],
    };
    d.capabilities = d.requirements();
    assert!(d.validate().is_err());
}
