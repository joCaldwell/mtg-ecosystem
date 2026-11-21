//! A resolution-only decision that admits or skips a bounded instruction block.
//! CR 608.2b–d/h, 701.23b, 701.24a.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use mtg_contracts::EffectRecipient;

impl Resolving {
    pub(super) fn entry_search_player(&self, game: &Game) -> Result<PlayerId, String> {
        match self.outcomes.last() {
            Some(InstructionOutcome::OptionalBlock {
                accepted_by: Some(player),
            }) => Ok(*player),
            _ => Ok(game.entry(self.stack_id)?.controller),
        }
    }
}

impl Game {
    pub(super) fn optional_recipient(
        &self,
        frame: &Resolving,
        recipient: &EffectRecipient,
    ) -> Result<Option<PlayerId>, String> {
        match recipient {
            EffectRecipient::Controller => Ok(Some(self.entry(frame.stack_id)?.controller)),
            EffectRecipient::TargetController { target_slot }
                if frame.legal_targets.get(*target_slot as usize) == Some(&true) =>
            {
                self.removal_target_controller(frame, *target_slot)
                    .map(Some)
            }
            EffectRecipient::TargetController { .. } => Ok(None),
        }
    }

    pub(super) fn begin_optional_block(
        &mut self,
        frame: Resolving,
        chooser: EffectRecipient,
        following: u32,
        source_block: u32,
    ) -> Result<(), String> {
        if let Some(player) = self.optional_recipient(&frame, &chooser)?
            && self.is_live(player)
        {
            self.state.rules.frame = Frame::ResolutionChoice {
                frame,
                choice: ResolutionChoice::ChooseOptional {
                    player,
                    source_block,
                },
            };
            return Ok(());
        }
        self.finish_optional_block(frame, following, None)
    }

    pub(super) fn choose_optional_block(
        &mut self,
        frame: Resolving,
        player: PlayerId,
        source_block: u32,
    ) -> Result<(), String> {
        let Some(Instruction::ChooseOptional {
            chooser,
            following,
            source_block: expected,
        }) = self
            .program_for(self.entry(frame.stack_id)?)?
            .instructions
            .get(frame.cursor as usize)
        else {
            return Err("missing optional instruction".into());
        };
        if *expected != source_block || self.optional_recipient(&frame, chooser)? != Some(player) {
            return Err("optional choice does not match its instruction".into());
        }
        let following = *following;
        let accepted_by = if self.is_live(player) {
            let input = self
                .state
                .rules
                .pending
                .take()
                .ok_or("missing optional choice")?;
            if input.player != player {
                return Err("wrong optional chooser".into());
            }
            let Response::OptionalEffect { perform } = input.response else {
                return Err("wrong optional response".into());
            };
            perform.then_some(player)
        } else {
            self.state.rules.pending = None;
            None
        };
        self.finish_optional_block(frame, following, accepted_by)
    }

    fn finish_optional_block(
        &mut self,
        mut frame: Resolving,
        following: u32,
        accepted_by: Option<PlayerId>,
    ) -> Result<(), String> {
        let next = frame
            .cursor
            .checked_add(1)
            .ok_or("optional instruction cursor limit")?;
        let end = next
            .checked_add(following)
            .ok_or("optional block size overflow")?;
        if following == 0
            || end as usize
                > self
                    .program_for(self.entry(frame.stack_id)?)?
                    .instructions
                    .len()
        {
            return Err("invalid optional block extent".into());
        }
        if accepted_by.is_some() {
            return self
                .finish_instruction(frame, InstructionOutcome::OptionalBlock { accepted_by });
        }
        frame
            .outcomes
            .push(InstructionOutcome::OptionalBlock { accepted_by: None });
        frame
            .outcomes
            .extend((next..end).map(|_| InstructionOutcome::OptionalSkipped));
        frame.cursor = end;
        self.state.rules.frame = Frame::Resolve { frame };
        Ok(())
    }
}
