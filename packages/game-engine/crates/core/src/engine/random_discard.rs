//! CR 107.3a/608.2b-d/701.9a-b: discard from the announced player's hand.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RandomDiscardProgress {
    // Canonical order records only membership, including exact incarnations.
    pub(super) selected: Vec<ObjectId>,
}

impl Game {
    fn random_discard_spec<'a>(
        &'a self,
        frame: &'a Resolving,
    ) -> Result<(PlayerId, &'a Quantity, u32), String> {
        if self.state.rules.match_state.is_none() {
            return Err("random discard requires a seeded match".into());
        }
        let program = self.program_for(&frame.spell)?;
        let Some(Instruction::DiscardTargetPlayerAtRandom {
            target_slot,
            count,
            source_block,
        }) = program.instructions.get(frame.cursor as usize)
        else {
            return Err("missing random discard instruction".into());
        };
        let slot = *target_slot as usize;
        if !frame.spell.kind.is_spell()
            || frame.spell.paid.as_ref() != Some(&frame.spell.total_cost)
            || frame.legal_targets.get(slot) != Some(&true)
            || !program.targets.get(slot).is_some_and(|spec| {
                matches!(
                    spec.domain,
                    TargetDomain::AnyPlayer | TargetDomain::OpponentPlayer
                )
            })
        {
            return Err("random discard requires a paid spell with a legal player target".into());
        }
        let player = *frame
            .spell
            .player_targets
            .get(slot)
            .ok_or("missing random discard target")?;
        let count = match count {
            RandomDiscardCount::Fixed { value } => value,
            RandomDiscardCount::AnnouncedX => {
                frame.spell.x.as_ref().ok_or("missing announced X")?
            }
        };
        if count.is_negative() {
            return Err("negative random discard count".into());
        }
        Ok((player, count, *source_block))
    }

    pub(super) fn validate_random_discard_progress(&self, frame: &Resolving) -> Result<(), String> {
        let Some(progress) = &frame.random_discard else {
            return Ok(());
        };
        let (player, count, _) = self.random_discard_spec(frame)?;
        let hand = self.action_hand(player);
        if !self.is_live(player)
            || !matches!(&self.state.rules.frame, Frame::Resolve { .. })
            || progress.selected.is_empty()
            || !progress.selected.windows(2).all(|pair| pair[0] < pair[1])
            || !progress
                .selected
                .iter()
                .all(|id| hand.binary_search(id).is_ok())
            || Quantity::from(progress.selected.len() as i64) > *count
            || *count >= Quantity::from(hand.len() as i64)
        {
            return Err("random discard progress invariant".into());
        }
        Ok(())
    }

    pub(super) fn random_discard_step(&mut self, mut frame: Resolving) -> Result<(), String> {
        let mut progress = frame.random_discard.take().unwrap_or_default();
        let (player, count, source_block) = self.random_discard_spec(&frame)?;
        if !self.is_live(player) {
            return self
                .finish_instruction(frame, InstructionOutcome::PlayerActionSkipped { player });
        }

        let mut hand = self.action_hand(player);
        let objects = if *count >= Quantity::from(hand.len() as i64) {
            hand
        } else if *count == Quantity::from(progress.selected.len() as i64) {
            progress.selected
        } else {
            // Each accepted word samples one rank among the remaining cards.
            // Every ordered sample is equiprobable, hence so is its subset.
            hand.retain(|id| progress.selected.binary_search(id).is_err());
            let upper = u32::try_from(hand.len()).map_err(|_| "random discard range overflow")?;
            if upper < 2 {
                return Err("invalid random discard sampling range".into());
            }
            let random = &mut self
                .state
                .rules
                .match_state
                .as_mut()
                .ok_or("random discard requires a seeded match")?
                .random;
            if let Some(index) = setup::sample_bounded_word(random.word()?, upper) {
                let id = hand[index as usize];
                let position = progress.selected.partition_point(|selected| *selected < id);
                progress.selected.insert(position, id);
            }
            if !progress.selected.is_empty() {
                frame.random_discard = Some(progress);
            }
            self.state.rules.frame = Frame::Resolve { frame };
            return Ok(());
        };

        let outcome =
            self.discard_batch(Self::wipe_cause(&frame, source_block), player, objects)?;
        self.finish_instruction(frame, outcome)
    }
}
