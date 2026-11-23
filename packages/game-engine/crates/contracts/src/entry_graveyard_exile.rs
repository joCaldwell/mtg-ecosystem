//! CR 400.7, 603.3d, 603.6a and 608.2b/c. The trigger targets one player.
//! The objects in that player's graveyard are determined on resolution.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const INVALID: &str = "requires a complete ordinary self-entry graveyard exile source";
const SELECTION: &str = "line:1:6";

fn abilities(land: bool) -> Value {
    let subject = json!({"ref":"this","noun":if land { "land" } else { "creature" }});
    let trigger = json!({"kind":"triggered","trigger":{"trigger":"enters","what":subject},
        "effects":[{"steps":[{"effect":"exile","what":{"ref":"zone-cards","zone":"graveyard",
        "owner":{"player":"target-player","selectionId":SELECTION}}}]}]});
    if land {
        json!([{"kind":"static","effect":{"static":"enters-tapped","what":subject}},trigger,
        {"kind":"activated","costs":[{"cost":"tap-self"}],"effects":[{"steps":[
            {"effect":"add-mana","who":{"player":"you"},"mana":{"mana":"fixed","symbols":["B"]}}
        ]}]}])
    } else {
        json!([{"kind":"keywords","keywords":[{"keyword":"flying"}]},trigger])
    }
}

fn trigger() -> TriggeredAbility {
    TriggeredAbility {
        functioning_zone: FunctioningZone::Battlefield,
        condition: TriggerCondition::SelfEntersBattlefield,
        intervening_if: None,
        death_filter: None,
        program: SpellProgram {
            functioning_zone: FunctioningZone::Stack,
            timing: SpellTiming::Instant,
            resolution: SpellResolution::Instructions,
            modes: vec![],
            additional_sacrifice: None,
            optional_additional_costs: vec![],
            targets: vec![TargetSpec {
                slot: 0,
                selection_id: SELECTION.into(),
                chooser: Chooser::Controller,
                stage: EvaluationStage::Announcement,
                domain: TargetDomain::AnyPlayer,
                filter: None,
            }],
            instructions: vec![Instruction::exile_target_graveyard(0, 1)],
        },
    }
}

fn enters_tapped() -> StaticAbility {
    StaticAbility {
        functioning_zone: FunctioningZone::Battlefield,
        effect: StaticEffect::EntersTapped,
    }
}
