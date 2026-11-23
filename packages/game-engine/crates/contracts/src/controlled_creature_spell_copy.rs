//! CR 601.2c, 608.2b, 707.9, 707.10 and 707.10f (2026-09-25).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const SELECTION: &str = "line:0:1";
const PREFIX: &str = "Copy target creature spell you control,";

const TEXT: &str = "Copy target creature spell you control, except it isn't legendary if the spell is legendary. (A copy of a creature spell becomes a token.)";
const AST_OP: &str = "copy-stack-object";
const INVALID: &str =
    "creature spell copy requires the complete ordinary instant and its exact nonlegendary copy";

fn abilities() -> Value {
    json!([{"kind":"spell","effects":[{"steps":[{
        "effect":AST_OP,
        "what":{"ref":"target","filter":{"cls":"spell","types":["creature"],"control":"you"},
            "count":{"amount":"fixed","value":1},"selectionId":SELECTION},
        "who":{"player":"you"},
        "count":{"amount":"fixed","value":1},
        "copyExceptions":[{"change":"types","types":{"supertypes":{"operation":"remove","types":["legendary"]}}}]
    }]}]}])
}

fn target() -> TargetSpec {
    TargetSpec {
        slot: 0,
        selection_id: SELECTION.into(),
        chooser: Chooser::Controller,
        stage: EvaluationStage::Announcement,
        domain: TargetDomain::ControlledCreatureSpell,
        filter: None,
    }
}
