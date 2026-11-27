//! CR 107.3a/608.2b-d/701.9a-b: discard from the announced player's hand,
//! or select matching cards of the controller's graveyard, at random.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use mtg_contracts::kir::SearchSpec;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RandomDiscardProgress {
    pub stream: u64,
    // Canonical order records only membership, including exact incarnations.
    pub selected: Vec<ObjectId>,
}

impl RulesTxn {
    /// The legacy random search is the only instruction-cursor sampler.
    /// Evaluated steps supply their checked player, count and pool directly.
    fn random_search_spec<'a>(
        &'a self,
        frame: &'a Resolving,
    ) -> Result<(PlayerId, &'a Quantity, u32, &'a SearchSpec), String> {
        if self.state.rules.match_state.is_none() {
            return Err("random selection requires a seeded match".into());
        }
        let entry = self.entry(frame.stack_id)?;
        if entry.paid.as_ref() != Some(&entry.total_cost) {
            return Err("random selection requires a paid invocation".into());
        }
        let program = self.program_for(entry)?;
        let Some(kernel!(Search { player: you!(), search } @ source_block)) =
            program.instructions.get(frame.cursor as usize)
        else {
            return Err("missing random search instruction".into());
        };
        if !search.random {
            return Err("missing random search mode".into());
        }
        let count = search
            .maximum
            .as_fixed()
            .ok_or("nonfixed random search count")?;
        Ok((entry.controller, count, *source_block, search))
    }

    pub(super) fn validate_seat_sample(
        &self,
        player: PlayerId,
        hand: &[ObjectId],
        count: &Quantity,
        progress: &RandomDiscardProgress,
    ) -> Result<(), String> {
        if !self.is_live(player)
            || !self.state.rules.match_state.as_ref().is_some_and(|s| {
                s.random.open.is_some_and(|p| p.stream == progress.stream)
                    && s.random.seat_random[player.0 as usize]
                        .checked_sub(1)
                        .and_then(|n| setup::stream_id(setup::SEAT_RANDOM, player, n).ok())
                        == Some(progress.stream)
            })
            || !progress.selected.windows(2).all(|pair| pair[0] < pair[1])
            || !progress
                .selected
                .iter()
                .all(|id| hand.binary_search(id).is_ok())
            || Quantity::from(progress.selected.len() as i64) > *count
            || *count >= Quantity::from(hand.len() as i64)
        {
            return Err("invalid random seat sample progress".into());
        }
        Ok(())
    }

    pub(super) fn validate_random_discard_progress(&self, frame: &Resolving) -> Result<(), String> {
        let Some(progress) = &frame.random_discard else {
            return Ok(());
        };
        let (player, count, _, search) = self.random_search_spec(frame)?;
        if !matches!(self.state.rules.frame, Frame::Resolve { .. }) {
            return Err("invalid random search frame".into());
        }
        self.validate_seat_sample(
            player,
            &self.search_bounds(player, search)?.candidates,
            count,
            progress,
        )
    }

    pub(super) fn random_discard_step(&mut self, mut frame: Resolving) -> Result<(), String> {
        let mut progress = frame.random_discard.take();
        let (player, count, source_block, search) = self.random_search_spec(&frame)?;
        if !self.is_live(player) {
            return self
                .finish_instruction(frame, InstructionOutcome::PlayerActionSkipped { player });
        }
        let pool = self.search_bounds(player, search)?.candidates;
        let count = count.clone();
        if let Some(objects) = self.advance_seat_sample(player, pool, &count, &mut progress)? {
            return self.complete_search(frame, player, objects, source_block);
        }
        frame.random_discard = progress;
        self.rules_mut().frame = Frame::Resolve { frame };
        Ok(())
    }

    /// Sample without replacement using at most one word of the seat's seeded
    /// stream per engine step. Progress and card identities stay trusted.
    pub(super) fn advance_seat_sample(
        &mut self,
        player: PlayerId,
        mut hand: Vec<ObjectId>,
        count: &Quantity,
        saved: &mut Option<RandomDiscardProgress>,
    ) -> Result<Option<Vec<ObjectId>>, String> {
        let ongoing = saved.is_some();
        let mut progress = saved.take().unwrap_or_default();
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
                .rules_mut()
                .match_state
                .as_mut()
                .ok_or("random discard requires a seeded match")?
                .random;
            if !ongoing {
                let ordinal = &mut random.seat_random[player.0 as usize];
                progress.stream = setup::stream_id(setup::SEAT_RANDOM, player, *ordinal)?;
                *ordinal += 1;
            }
            if let Some(index) = setup::sample_bounded_word(random.word(progress.stream)?, upper) {
                let id = hand[index as usize];
                let position = progress.selected.partition_point(|selected| *selected < id);
                progress.selected.insert(position, id);
            }
            *saved = Some(progress);
            return Ok(None);
        };

        if ongoing {
            self.rules_mut().match_state.as_mut().unwrap().random.open = None;
        }
        Ok(Some(objects))
    }
}
