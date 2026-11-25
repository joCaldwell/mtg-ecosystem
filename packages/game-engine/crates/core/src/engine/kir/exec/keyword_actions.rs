//! CR 701.34, 701.36: choices read the current public board at resolution.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use crate::engine::*;

impl RulesTxn {
    pub(in crate::engine) fn is_controlled_creature_token(
        &self,
        player: PlayerId,
        object: ObjectId,
    ) -> bool {
        self.is_battlefield_creature(object)
            && self.current(object).is_ok_and(|o| {
                o.controller == player
                    && self
                        .state
                        .rules
                        .tokens
                        .get(&object.card)
                        .is_some_and(|t| t.is_token(o))
            })
    }

    pub(in crate::engine) fn keyword_action_request(
        &self,
        frame: &Resolving,
        choice: &ResolutionChoice,
    ) -> Result<Option<Decision<Rules>>, String> {
        let player = self.entry(frame.stack_id)?.controller;
        if !self.is_live(player) {
            return Ok(None);
        }
        let (reason, min, candidates) = match choice {
            ResolutionChoice::Populate => (
                Reason::Populate,
                1,
                self.state
                    .rules
                    .objects
                    .values()
                    .filter(|o| self.is_controlled_creature_token(player, o.id))
                    .map(|o| Candidate::Object { object: o.id })
                    .collect::<Vec<_>>(),
            ),
            ResolutionChoice::Proliferate => (
                Reason::Proliferate,
                0,
                self.state
                    .rules
                    .objects
                    .values()
                    .filter(|o| self.can_proliferate_permanent(o.id))
                    .map(|o| Candidate::Object { object: o.id })
                    .chain(
                        self.live_players()
                            .into_iter()
                            .filter(|player| self.can_proliferate_player(*player))
                            .map(|player| Candidate::Player { player }),
                    )
                    .collect(),
            ),
            _ => return Err("not a keyword action choice".into()),
        };
        Ok((!candidates.is_empty()).then(|| {
            let max = if min == 1 { 1 } else { candidates.len() as u32 };
            self.pick(player, reason, candidates, min, max)
        }))
    }

    pub(in crate::engine) fn begin_keyword_choice(
        &mut self,
        frame: Resolving,
        choice: ResolutionChoice,
    ) -> Result<(), String> {
        if self.keyword_action_request(&frame, &choice)?.is_none() {
            return self.finish_instruction(frame, InstructionOutcome::Effect { actual: None });
        }
        self.rules_mut().frame = Frame::ResolutionChoice { frame, choice };
        Ok(())
    }

    pub(in crate::engine) fn keyword_action_step(
        &mut self,
        frame: Resolving,
        choice: ResolutionChoice,
    ) -> Result<(), String> {
        let Some(request) = self.keyword_action_request(&frame, &choice)? else {
            self.rules_mut().pending = None;
            return self.finish_instruction(frame, InstructionOutcome::Effect { actual: None });
        };
        let input = self
            .rules_mut()
            .pending
            .take()
            .ok_or("missing keyword action choice")?;
        let entry = self.entry(frame.stack_id)?;
        let source = entry.source;
        let instruction = self
            .program_for(entry)?
            .instructions
            .get(frame.cursor as usize)
            .ok_or("missing keyword action instruction")?;
        let cause = self.instruction_cause(&frame, instruction.source_block())?;
        match choice {
            ResolutionChoice::Populate => {
                let actual = self.commit(
                    ProposedEvent::CreateTokenCopy {
                        controller: request.player,
                        source,
                        original: input.object()?,
                    },
                    cause,
                )?;
                self.finish_instruction(
                    frame,
                    InstructionOutcome::Effect {
                        actual: Some(actual),
                    },
                )
            }
            ResolutionChoice::Proliferate => self.proliferate(frame, input.entities()?, cause),
            _ => Err("not a keyword action choice".into()),
        }
    }
}
