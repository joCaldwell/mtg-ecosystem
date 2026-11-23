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
