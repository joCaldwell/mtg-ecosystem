//! CR 106.6, 109.2, 109.2b, 305.6 and 614.12 (2026-09-25).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const ENTRY: &str = "This land enters tapped unless you control a Forest.";
const FREE: &str = "{T}: Add {G}.";
const PAID: &str = "{2}{G}{G}, {T}: Add six {G}. Spend this mana only to cast creature spells or activate abilities of creatures.";

pub fn source(text: &str) -> Option<Value> {
    if !text.lines().eq([ENTRY, FREE, PAID]) {
        return None;
    }
    Some(json!([
        {"kind":"static","effect":{"static":"enters-tapped","what":{"ref":"this","noun":"land"},
            "conditionKind":"unless","condition":{"condition":"control","who":{"player":"you"},
                "what":query()}}},
        {"kind":"activated","costs":[{"cost":"tap-self"}],"effects":[{"steps":[{
            "effect":"add-mana","who":{"player":"you"},"mana":{"mana":"fixed","symbols":["G"]}
        }]}]},
        {"kind":"activated","costs":[{"cost":"mana","symbols":["2","G","G"]},{"cost":"tap-self"}],
            "effects":[{"steps":[{
                "effect":"add-mana","who":{"player":"you"},"mana":{"mana":"fixed","symbols":["G","G","G","G","G","G"]},
                "restriction":{"restriction":"only","uses":[
                    {"use":"cast","filter":{"types":["creature"],"cls":"spell"}},
                    {"use":"activate","source":{"types":["creature"]}}
                ]}
            }]}]}
    ]))
}

pub fn query() -> Value {
    json!({"ref":"filter","filter":{"subtypes":["Forest"]},"count":{"amount":"fixed","value":1}})
}

pub fn entry() -> StaticAbility {
    StaticAbility {
        functioning_zone: FunctioningZone::Battlefield,
        effect: StaticEffect::EntersTappedUnlessControllerLandType {
            subtype: BasicLandType::Forest,
        },
    }
}

pub fn abilities() -> [ManaAbility; 2] {
    let tap = PermanentActivationCosts {
        tap: true,
        mana: None,
        sacrifice: false,
        remove_source_plus_one_counters: None,
    };
    [
        ManaAbility {
            self_damage: None,
            restriction: None,
            costs: tap.clone(),
            production: ManaProduction::Fixed {
                symbols: vec![Color::G],
            },
            source_block: 1,
        },
        ManaAbility {
            self_damage: None,
            restriction: None,
            costs: PermanentActivationCosts {
                mana: Some(ManaCost {
                    generic: 2.into(),
                    colored: vec![Color::G; 2],
                }),
                ..tap
            },
            production: ManaProduction::RestrictedFixed {
                symbols: vec![Color::G; 6],
                spend: ManaSpendRestriction::CreatureSpellOrBattlefieldCreatureAbility,
            },
            source_block: 2,
        },
    ]
}

pub fn present(d: &Definition) -> bool {
    d.static_abilities.iter().any(|a| {
        matches!(
            a.effect,
            StaticEffect::EntersTappedUnlessControllerLandType { .. }
        )
    }) || d.mana_abilities.iter().any(|a| {
        matches!(
            a.production,
            ManaProduction::RestrictedFixed {
                spend: ManaSpendRestriction::CreatureSpellOrBattlefieldCreatureAbility,
                ..
            }
        )
    })
}

pub(super) fn validate(d: &Definition) -> Result<bool, String> {
    if !present(d) && !d.provenance.source_json.contains(PAID) {
        return Ok(false);
    }
    let bad = "requires complete Forest entry and separate printed free and restricted paid mana";
    let raw: Value = strict_read(&d.provenance.source_json)?;
    let text = raw["oracle_text"].as_str().ok_or(bad)?;
    let ast = source(text).ok_or(bad)?;
    let c = &d.characteristics;
    if c.kind != CardKind::Land
        || c.basic
        || c.legendary
        || c.snow
        || !c.additional_types.is_empty()
        || !c.subtypes.is_empty()
        || !c.colors.is_empty()
        || c.color_identity != [Color::G]
        || !c.hybrid_mana.is_empty()
        || d.x_mana_cost
        || d.intrinsic_zone != Some(FunctioningZone::Battlefield)
        || d.spell.is_some()
        || !d.activated.is_empty()
        || !d.triggered.is_empty()
        || !d.keywords.is_empty()
        || d.mana_abilities != abilities()
        || d.static_abilities != [entry()]
        || raw["type_line"] != "Land"
        || raw["mana_cost"] != ""
        || raw["layout"] != "normal"
        || raw["oracle_id"] != d.oracle_id
        || raw["name"] != d.name
        || raw["id"] != d.provenance.printing_id
        || raw["colors"] != json!([])
        || raw["color_identity"] != json!(["G"])
        || raw["produced_mana"] != json!(["G"])
        || raw["keywords"] != json!([])
        || strict_read::<Value>(&d.provenance.abilities_json)? != ast
        || d.source_labels.as_ref().is_some_and(|l| {
            l.text != text
                || l.blocks != [ENTRY, FREE, PAID]
                || l.mana != [1, 2]
                || l.statics != [0]
                || !l.activated.is_empty()
                || !l.triggered.is_empty()
                || !l.spell_modes.is_empty()
        })
    {
        return Err(bad.into());
    }
    Ok(true)
}

pub(super) fn schema_candidate() -> Value {
    json!({"anyOf":[
        {"properties":{"provenance":{"properties":{"source_json":{"pattern":PAID
            .replace('{', r"\{").replace('}', r"\}").replace('.', r"\.")}}}}},
        {"required":["static_abilities"],"properties":{"static_abilities":{"contains":{"required":["effect"],"properties":{
            "effect":{"type":"object","required":["enters_tapped_unless_controller_land_type"]}
        }}}}},
        {"required":["mana_abilities"],"properties":{"mana_abilities":{"contains":{"properties":{
            "production":{"required":["spend"],"properties":{"spend":{
                "const":ManaSpendRestriction::CreatureSpellOrBattlefieldCreatureAbility}}
            }
        }}}}}
    ]})
}

pub(super) fn extend_schema(schema: &mut Value) {
    use Capability::*;
    let caps = [
        NormalLayoutV1,
        LandPlayV1,
        PermanentManaV1,
        RestrictedManaV1,
    ];
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "if":schema_candidate(),
        "then":{"required":["static_abilities","mana_abilities","intrinsic_zone"],"properties":{
            "capabilities":{"items":{"enum":caps},"allOf":caps.map(|c|json!({"contains":{"const":c}}))},
            "characteristics":{"properties":{"kind":{"const":"land"},"basic":{"const":false},
                "legendary":{"const":false},"snow":{"const":false},"subtypes":{"maxItems":0},
                "colors":{"maxItems":0},"color_identity":{"const":["G"]},"additional_types":{"maxItems":0},
                "hybrid_mana":{"maxItems":0}}},
            "x_mana_cost":{"const":false},"spell":{"type":"null"},
            "intrinsic_zone":{"const":"battlefield"},"keywords":{"maxItems":0},
            "activated":{"maxItems":0},"triggered":{"maxItems":0},
            "static_abilities":{"const":[entry()]},"mana_abilities":{"const":abilities()}
        }}
    }));
}
