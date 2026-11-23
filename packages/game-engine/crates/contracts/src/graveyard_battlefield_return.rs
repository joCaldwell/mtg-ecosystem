//! CR 109.5, 110.2a, 400.7, 601.2c and 608.2b–c.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const RETURN: &str = "Return target creature card from your graveyard to the battlefield";
const SELECTION: &str = "line:0:1";
const INVALID: &str = "graveyard battlefield return requires its complete ordinary sorcery";

fn target() -> TargetSpec {
    TargetSpec {
        slot: 0,
        selection_id: SELECTION.into(),
        chooser: Chooser::Controller,
        stage: EvaluationStage::Announcement,
        domain: TargetDomain::OwnGraveyardCreatureCard,
        filter: None,
    }
}

fn source_effects(gain: Option<u32>) -> (String, Value) {
    let mut text = format!("{RETURN}.");
    let mut effects = vec![
        json!({"steps":[{"effect":"move-zone","to":{"zone":"battlefield"},
            "what":{"ref":"target","filter":{"types":["creature"],"cls":"card","zone":{"zone":"graveyard","owner":"your"}},
                    "count":{"amount":"fixed","value":1},"selectionId":SELECTION}
        }]}),
    ];
    if let Some(value) = gain {
        text.push_str(&format!(" You gain {value} life."));
        effects.push(json!({"steps":[{"effect":"gain-life","who":{"player":"you"},"amount":{"amount":"fixed","value":value}}]}));
    }
    (text, json!([{"kind":"spell","effects":effects}]))
}
