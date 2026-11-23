//! Persistent land allowance, own-graveyard land play and controlled-entry mill.
//! CR 305.1–3, 611.3, 603.3b/603.6a and 701.17 (2026-09-25).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const TRIGGER: &str = "Landfall — Whenever a land you control enters, mill a card.";
const INVALID: &str =
    "requires the complete persistent land allowance, graveyard permission and land-entry mill";

pub(super) fn present(d: &Definition) -> bool {
    d.static_abilities.iter().any(|a| {
        matches!(
            a.effect,
            StaticEffect::AdditionalLandPlay | StaticEffect::PlayLandsFromOwnGraveyard
        )
    })
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

pub(super) fn selector() -> Value {
    json!({"anyOf":[
        {"required":["static_abilities"],"properties":{"static_abilities":{"contains":{
            "required":["effect"],"properties":{"effect":{"enum":[StaticEffect::AdditionalLandPlay,StaticEffect::PlayLandsFromOwnGraveyard]}}
        }}}},
        {"properties":{"provenance":{"properties":{"source_json":{"pattern":r#""oracle_text"\s*:\s*"You may play an additional land on each of your turns\.\\nYou may play lands from your graveyard\."#}}}}}
    ]})
}
