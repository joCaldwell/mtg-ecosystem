//! A creature-or-planeswalker union remains explicit even though the current
//! definition type system cannot construct a planeswalker. CR 109.2, 115.1a.
use super::*;
use serde_json::{Value, json};

pub const CLAUSE: &str = "Return target creature or planeswalker to its owner's hand.";

pub fn program(text: &str, step: &Value, timing: SpellTiming, block: u32) -> Option<SpellProgram> {
    if text != CLAUSE {
        return None;
    }
    let id = step["what"]["selectionId"].as_str()?;
    let expected = json!({"effect":"return-to-hand","what":{"ref":"target","filter":{"allOf":[{"anyOf":[{"types":["creature"]},{"types":["planeswalker"]}]}]},"count":{"amount":"fixed","value":1},"selectionId":id},"to":"owner"});
    if id.is_empty() || step != &expected {
        return None;
    }
    Some(SpellProgram {
        timing,
        resolution: SpellResolution::Instructions,
        functioning_zone: FunctioningZone::Stack,
        targets: vec![TargetSpec {
            slot: 0,
            selection_id: id.into(),
            chooser: Chooser::Controller,
            stage: EvaluationStage::Announcement,
            domain: TargetDomain::CreatureOrPlaneswalker,
            filter: None,
        }],
        instructions: vec![Instruction::move_target(
            0,
            kir::Destination::OwnerHand,
            block,
        )],
        modes: vec![],
        additional_sacrifice: None,
        optional_additional_costs: vec![],
    })
}
fn uses(p: &SpellProgram) -> bool {
    p.targets
        .iter()
        .any(|s| s.domain == TargetDomain::CreatureOrPlaneswalker)
        || p.modes.iter().any(|m| uses(&m.program))
}
