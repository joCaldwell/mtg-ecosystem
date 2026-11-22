//! CR121.2/401.4/400.7/608.2c/903.9b: counted draws, then ordered hand return.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
impl Game {
    pub(super) fn private_library_choice_candidates(
        &self,
        viewer: PlayerId,
    ) -> Option<&[ObjectId]> {
        if let Frame::ResolutionChoice {
            choice: ResolutionChoice::HandReturn { player, hand, .. },
            ..
        } = &self.state.rules.frame
            && *player == viewer
            && self.is_live(viewer)
        {
            Some(hand)
        } else {
            self.library_looked(viewer)
        }
    }
    fn hand_return_instruction(&self, frame: &Resolving) -> Result<(Quantity, u32), String> {
        match self
            .program_for(self.entry(frame.stack_id)?)?
            .instructions
            .get(frame.cursor as usize)
        {
            Some(Instruction::PutHandOnLibrary {
                count,
                source_block,
            }) if !count.is_negative() => Ok((count.clone(), *source_block)),
            _ => Err("hand return instruction invariant".into()),
        }
    }
    pub(super) fn bounded_hand_count(&self, player: PlayerId, count: &Quantity) -> usize {
        let len = self.action_hand(player).len();
        (0..len)
            .take_while(|n| Quantity::from(*n as i64) < *count)
            .count()
    }
    pub(super) fn hand_return_count_for_current(&self) -> Result<usize, String> {
        let Frame::ResolutionChoice {
            frame,
            choice: ResolutionChoice::HandReturn { player, .. },
        } = &self.state.rules.frame
        else {
            return Err("hand return frame invariant".into());
        };
        let (count, _) = self.hand_return_instruction(frame)?;
        Ok(self.bounded_hand_count(*player, &count))
    }
    pub(super) fn validate_hand_return_source(
        &self,
        frame: &Resolving,
        player: PlayerId,
        block: u32,
    ) -> Result<(), String> {
        let (_, b) = self.hand_return_instruction(frame)?;
        if player != self.entry(frame.stack_id)?.controller || b != block {
            return Err("hand return source binding".into());
        }
        Ok(())
    }
    pub(super) fn validate_hand_return(
        &self,
        frame: &Resolving,
        player: PlayerId,
        block: u32,
        objects: &[ObjectId],
    ) -> Result<(), String> {
        self.validate_hand_return_source(frame, player, block)?;
        let (count, _) = self.hand_return_instruction(frame)?;
        if !self.is_live(player)
            || objects.len() != self.bounded_hand_count(player, &count)
            || objects.iter().collect::<BTreeSet<_>>().len() != objects.len()
            || objects.iter().any(|o| {
                !self
                    .current(*o)
                    .is_ok_and(|v| v.zone == (Zone::Hand { owner: player }))
            })
        {
            return Err("invalid ordered hand return".into());
        }
        Ok(())
    }
    pub(super) fn begin_hand_return(
        &mut self,
        frame: Resolving,
        count: Quantity,
        block: u32,
    ) -> Result<(), String> {
        let player = self.entry(frame.stack_id)?.controller;
        if !self.is_live(player) {
            return self
                .finish_instruction(frame, InstructionOutcome::PlayerActionSkipped { player });
        }
        if count.is_negative() {
            return Err("negative hand return count".into());
        }
        if self.bounded_hand_count(player, &count) == 0 {
            return self.finish_instruction(frame, InstructionOutcome::Effect { actual: None });
        }
        self.rules_mut().frame = Frame::ResolutionChoice {
            frame,
            choice: ResolutionChoice::HandReturn {
                player,
                hand: self.action_hand(player),
                source_block: block,
            },
        };
        Ok(())
    }
    pub(super) fn hand_return_step(
        &mut self,
        frame: Resolving,
        player: PlayerId,
        block: u32,
    ) -> Result<(), String> {
        self.validate_hand_return_source(&frame, player, block)?;
        if !self.is_live(player) {
            if let Some(input) = self.rules_mut().pending.take()
                && (input.player != player
                    || !matches!(input.response, Response::HandReturn { .. }))
            {
                return Err("departed hand return input".into());
            }
            return self
                .finish_instruction(frame, InstructionOutcome::PlayerActionSkipped { player });
        }
        let input = self
            .rules_mut()
            .pending
            .take()
            .ok_or("missing hand return input")?;
        let Response::HandReturn { objects } = input.response else {
            return Err("hand return input kind".into());
        };
        if input.player != player {
            return Err("hand return actor".into());
        }
        self.validate_hand_return(&frame, player, block, &objects)?;
        let remaining = objects
            .iter()
            .filter(|o| self.state.rules.commanders.history(o.card).is_some())
            .copied()
            .collect();
        self.continue_hand_return(frame, objects, remaining, vec![], block)
    }
    pub(super) fn continue_hand_return(
        &mut self,
        frame: Resolving,
        objects: Vec<ObjectId>,
        remaining: Vec<ObjectId>,
        choices: Vec<(ObjectId, bool)>,
        block: u32,
    ) -> Result<(), String> {
        let player = self.entry(frame.stack_id)?.controller;
        if !self.is_live(player) {
            return self
                .finish_instruction(frame, InstructionOutcome::PlayerActionSkipped { player });
        }
        if let Some(object) = remaining.first().copied() {
            self.rules_mut().frame = Frame::CommanderReturn {
                object,
                destination: Zone::Library { owner: player },
                continuation: CommanderContinuation::HandReturn {
                    frame,
                    objects,
                    remaining,
                    choices,
                    source_block: block,
                },
            };
            return Ok(());
        }
        self.validate_hand_return(&frame, player, block, &objects)?;
        let (count, _) = self.hand_return_instruction(&frame)?;
        let command = choices
            .into_iter()
            .filter_map(|(o, yes)| yes.then_some(o))
            .collect();
        let actual = self.commit(
            ProposedEvent::HandReturn {
                player,
                count,
                objects,
                command,
            },
            BatchCause::Instruction {
                stack_id: frame.stack_id,
                player,
                source: self.entry(frame.stack_id)?.source,
                cursor: frame.cursor,
                source_block: block,
            },
        )?;
        self.finish_instruction(
            frame,
            InstructionOutcome::Effect {
                actual: Some(actual),
            },
        )
    }
    pub(super) fn validate_hand_return_continuation(
        &self,
        frame: &Resolving,
        objects: &[ObjectId],
        remaining: &[ObjectId],
        choices: &[(ObjectId, bool)],
        block: u32,
    ) -> Result<(), String> {
        self.validate_resolving(frame)?;
        self.validate_hand_return(
            frame,
            self.entry(frame.stack_id)?.controller,
            block,
            objects,
        )?;
        let expected = objects
            .iter()
            .filter(|o| self.state.rules.commanders.history(o.card).is_some())
            .copied()
            .collect::<Vec<_>>();
        if choices.len() + remaining.len() != expected.len()
            || choices
                .iter()
                .map(|(o, _)| *o)
                .chain(remaining.iter().copied())
                .collect::<Vec<_>>()
                != expected
        {
            return Err("hand return replacement prefix".into());
        }
        Ok(())
    }
    pub(super) fn apply_hand_return(
        &mut self,
        proposed: &ProposedEvent,
        before: Option<&PreEventFacts>,
    ) -> Result<EventOutcome, String> {
        let ProposedEvent::HandReturn {
            player,
            count,
            objects,
            command,
        } = proposed
        else {
            return Err("hand return event kind".into());
        };
        let player = *player;
        if !self.is_live(player)
            || count.is_negative()
            || objects.len() != self.bounded_hand_count(player, count)
            || objects.iter().collect::<BTreeSet<_>>().len() != objects.len()
            || objects.iter().any(|o| {
                !self
                    .current(*o)
                    .is_ok_and(|v| v.zone == (Zone::Hand { owner: player }))
            })
            || command.iter().collect::<BTreeSet<_>>().len() != command.len()
            || command.iter().any(|o| {
                !objects.contains(o) || self.state.rules.commanders.history(o.card).is_none()
            })
        {
            return Err("invalid hand return event".into());
        }
        let tail = self
            .state
            .rules
            .libraries
            .get(&player)
            .cloned()
            .unwrap_or_default();
        let mut movements = vec![];
        for object in objects {
            let actual = self.apply_occurrence_with_prior(
                &ProposedEvent::Move {
                    object: *object,
                    destination: if command.contains(object) {
                        Zone::Command { owner: player }
                    } else {
                        Zone::Library { owner: player }
                    },
                },
                before,
            )?;
            let EventOutcome::Moved { movement } = actual else {
                return Err("hand return actual movement".into());
            };
            movements.push(movement);
        }
        let top = movements
            .iter()
            .filter(|m| m.destination == (Zone::Library { owner: player }))
            .map(|m| m.successor)
            .collect::<Vec<_>>();
        self.rules_mut()
            .libraries
            .insert(player, top.iter().map(|o| o.card).chain(tail).collect());
        self.remember_library_order(player, &top)?;
        Ok(EventOutcome::HandReturned { player, movements })
    }
}
