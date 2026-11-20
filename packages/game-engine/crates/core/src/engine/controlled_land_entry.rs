//! CR 603.2c, 603.3b, 603.6a and 603.10 (2026-09-25).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

pub(super) fn matches(entrant: &ObjectBefore, controller: PlayerId) -> bool {
    entrant.zone == Zone::Battlefield
        && entrant.controller == controller
        && entrant.characteristics.has_type(CardKind::Land)
}

pub(super) fn entry_witness(
    entrant: &ObjectBefore,
    witness: &TriggerEventWitness,
) -> Result<TriggerEventWitness, String> {
    let movement = match witness {
        TriggerEventWitness::Moved { movement } => Some(movement.clone()),
        TriggerEventWitness::TokenEntered { .. } => None,
        _ => return Err("missing actual land entry movement".into()),
    };
    Ok(TriggerEventWitness::ControlledLandEntered {
        movement,
        entrant: Box::new(entrant.clone()),
    })
}
