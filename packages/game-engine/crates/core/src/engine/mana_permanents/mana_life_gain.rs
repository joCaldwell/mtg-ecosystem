//! Immediate life gain from a mana ability and its exact reversal.
//! CR 119.7, 605.3b, 601.2h and 733.1–2 (2026-09-25).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManaLifeUndo {
    before: Quantity,
    after: Quantity,
}

pub(super) fn actual(
    receipt: &CommittedBatch,
    index: u32,
    payer: PlayerId,
) -> Result<Option<ManaLifeUndo>, String> {
    match receipt.result(&[index]) {
        Some(EventOutcome::LifeGained {
            player,
            previous,
            current,
            ..
        }) if *player == payer => Ok(Some(ManaLifeUndo {
            before: previous.clone(),
            after: current.clone(),
        })),
        Some(EventOutcome::LifeChangeProhibited { player }) if *player == payer => Ok(None),
        _ => Err("unbound mana life-gain occurrence".into()),
    }
}

impl Game {
    pub(super) fn reverse_mana_life(
        &mut self,
        player: PlayerId,
        entry: &ManaLifeUndo,
    ) -> Result<(), String> {
        let life = &mut self.rules_mut().players[player.0 as usize].life;
        if *life != entry.after || entry.after <= entry.before {
            return Err("mana life gain changed".into());
        }
        // The gain is canceled, with no gameplay loss or replacement.
        *life = entry.before.clone();
        Ok(())
    }
}
