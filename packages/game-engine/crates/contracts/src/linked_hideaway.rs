//! CR 305.2–3, 406.3, 607.2a/3, 608.2g–h and 702.75a: linked face-down exile.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const HIDEAWAY: &str = "Hideaway 4 (When this land enters, look at the top four cards of your library, exile one face down, then put the rest on the bottom in a random order.)";
const ENTRY: &str = "This land enters tapped.";
const MANA: &str = "{T}: Add {G}.";
const PLAY: &str = "{G}, {T}: You may play the exiled card without paying its mana cost if creatures you control have total power 10 or greater.";
const INVALID: &str = "linked hideaway requires the complete tapped land and its two printed costs";

pub fn source(text: &str) -> bool {
    text.lines().eq([HIDEAWAY, ENTRY, MANA, PLAY]) && !text.ends_with('\n')
}

pub fn abilities() -> Value {
    json!([
        {"kind":"keywords","keywords":[{"keyword":"hideaway","amount":4}]},
        {"kind":"static","effect":{"static":"enters-tapped","what":{"ref":"this","noun":"land"}}},
        {"kind":"activated","costs":[{"cost":"tap-self"}],
            "effects":[{"steps":[{"effect":"add-mana","who":{"player":"you"},"mana":{"mana":"fixed","symbols":["G"]}}]}]},
        {"kind":"activated","costs":[{"cost":"mana","symbols":["G"]},{"cost":"tap-self"}],
            "effects":[{"steps":[{"effect":"play-card","optional":true,"instruction":{
                "who":{"player":"you"},"action":"play",
                "what":{"ref":"related","number":"singular","filter":{"cls":"card"},
                    "relation":{"relation":"event-result","event":"exile","scope":"contextual"}},
                "alternativeCost":{"cost":"without-mana-cost"},
                "condition":{"condition":"compare","value":{"amount":"attribute-sum","attribute":"power",
                    "among":{"ref":"each","filter":{"types":["creature"],"control":"you"}}},
                    "comparison":{"op":"ge","value":{"amount":"fixed","value":10}}}
            }}]}]}
    ])
}

fn program(instruction: Instruction) -> SpellProgram {
    SpellProgram {
        optional_additional_costs: vec![],
        additional_sacrifice: None,
        timing: SpellTiming::Instant,
        resolution: SpellResolution::Instructions,
        functioning_zone: FunctioningZone::Stack,
        targets: vec![],
        instructions: vec![instruction],
        modes: vec![],
    }
}

fn tap_cost(mana: Option<ManaCost>) -> PermanentActivationCosts {
    PermanentActivationCosts {
        remove_source_plus_one_counters: None,
        tap: true,
        mana,
        sacrifice: false,
    }
}

pub fn mana() -> ManaAbility {
    ManaAbility {
        self_damage: None,
        restriction: None,
        costs: tap_cost(None),
        production: ManaProduction::Fixed {
            symbols: vec![Color::G],
        },
        source_block: 2,
    }
}

pub fn tapped() -> StaticAbility {
    StaticAbility {
        functioning_zone: FunctioningZone::Battlefield,
        effect: StaticEffect::EntersTapped,
    }
}
