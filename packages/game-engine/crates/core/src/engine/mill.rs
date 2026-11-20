//! CR701.17/400.7/608.2b-c/903.9a: fixed library-to-graveyard operations.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
impl Game {
    pub(super) fn move_library_cards_to_graveyard(
        &mut self,
        player: PlayerId,
        objects: &[ObjectId],
        before: Option<&PreEventFacts>,
        replacement: Option<&BTreeMap<ObjectId, Zone>>,
    ) -> Result<Vec<MoveOutcome>, String> {
        let mut movements = Vec::with_capacity(objects.len());
        for object in objects {
            if self.current(*object)?.zone != (Zone::Library { owner: player }) {
                return Err("library movement source invariant".into());
            }
            let destination = replacement
                .and_then(|proof| proof.get(object).copied())
                .unwrap_or(Zone::Graveyard { owner: player });
            if destination == (Zone::Library { owner: player }) {
                continue;
            }
            let actual = self.apply_occurrence_with_prior(
                &ProposedEvent::Move {
                    object: *object,
                    destination,
                },
                before,
            )?;
            let EventOutcome::Moved { movement } = actual else {
                return Err("library graveyard movement invariant".into());
            };
            movements.push(movement);
        }
        // These exact successors are now public. Allocation follows the public move
        // order, never hidden physical IDs; the enclosing staged batch owns rollback.
        for movement in &movements {
            self.capture_trigger_source_handles(movement.successor)?;
        }
        Ok(movements)
    }
    pub(super) fn execute_mill(
        &mut self,
        frame: Resolving,
        recipient: Option<LifeRecipient>,
        count: Quantity,
        source_block: u32,
    ) -> Result<(), String> {
        let player = match recipient {
            None => frame.spell.controller,
            Some(LifeRecipient::Target { target_slot }) => {
                if frame.legal_targets.get(target_slot as usize) != Some(&true) {
                    return self
                        .finish_instruction(frame, InstructionOutcome::Effect { actual: None });
                }
                *frame
                    .spell
                    .player_targets
                    .get(target_slot as usize)
                    .ok_or("missing mill player target")?
            }
            _ => return Err("unsupported mill recipient".into()),
        };
        if self.is_live(player) && count > Quantity::zero() {
            let cards = self.library_prefix(player, &count)?;
            return self.complete_library_effect(
                frame,
                ProposedEvent::Mill {
                    player,
                    count,
                    cards,
                },
                source_block,
            );
        }
        let outcome = self.commit_mill(player, count, Self::staple_cause(&frame, source_block))?;
        self.finish_instruction(frame, outcome)
    }
    pub(super) fn commit_mill(
        &mut self,
        player: PlayerId,
        count: Quantity,
        cause: BatchCause,
    ) -> Result<InstructionOutcome, String> {
        if !self.is_live(player) {
            return Ok(InstructionOutcome::PlayerActionSkipped { player });
        }
        if count.is_negative() {
            return Err("negative mill count".into());
        }
        if count == Quantity::zero() {
            return Ok(InstructionOutcome::Effect { actual: None });
        }
        let cards = self.library_prefix(player, &count)?;
        let actual = self.commit(
            ProposedEvent::Mill {
                player,
                count,
                cards,
            },
            cause,
        )?;
        Ok(InstructionOutcome::Effect {
            actual: Some(actual),
        })
    }
    pub(super) fn apply_mill(
        &mut self,
        player: PlayerId,
        count: &Quantity,
        cards: &[ObjectId],
        before: Option<&PreEventFacts>,
        replacement: Option<&BTreeMap<ObjectId, Zone>>,
    ) -> Result<EventOutcome, String> {
        if !self.is_live(player)
            || count <= &Quantity::zero()
            || self.library_prefix(player, count)? != cards
        {
            return Err("invalid mill prefix".into());
        }
        let movements = self.move_library_cards_to_graveyard(player, cards, before, replacement)?;
        self.remember_mill(player, movements.len())?;
        Ok(EventOutcome::Milled { player, movements })
    }
}
