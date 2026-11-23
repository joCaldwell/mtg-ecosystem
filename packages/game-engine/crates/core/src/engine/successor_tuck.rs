use super::*;

// CR 109.5/603.3a/603.10a: the actual move and prior source controller
// determine whether "your graveyard" matches. CR 400.7e/603.6c track exactly
// the successor of that public move; do not find a later incarnation.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
pub(super) fn self_graveyard_successor(
    witness: &TriggerEventWitness,
    source: ObjectId,
    controller: PlayerId,
) -> Option<ObjectId> {
    let TriggerEventWitness::Moved { movement } = witness else {
        return None;
    };
    (movement.previous == source
        && movement.from == Zone::Battlefield
        && movement.destination == (Zone::Graveyard { owner: controller }))
    .then_some(movement.successor)
}
