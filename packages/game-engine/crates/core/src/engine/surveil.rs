//! CR 109.5,400.7,701.25a-d,903.9a: one completed Surveil operation
//! includes ordinary graveyard movement; Commander SBAs follow spell resolution.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
impl Game {
    pub(super) fn validate_surveil_choice(
        &self,
        frame: &Resolving,
        player: PlayerId,
        looked: &[ObjectId],
        source_block: u32,
    ) -> Result<(), String> {
        let program = self.program_for(&frame.spell)?;
        let Some(Instruction::Surveil {
            amount,
            source_block: expected,
        }) = program.instructions.get(frame.cursor as usize)
        else {
            return Err("snapshot surveil instruction binding".into());
        };
        if player != frame.spell.controller
            || source_block != *expected
            || amount <= &Quantity::zero()
            || looked.is_empty()
            || looked.len() > self.state.rules.cards.len()
            || looked.iter().collect::<BTreeSet<_>>().len() != looked.len()
            || looked
                .iter()
                .any(|o| !self.state.rules.cards.contains_key(&o.card))
            || (self.is_live(player) && self.library_prefix(player, amount)? != looked)
        {
            return Err("snapshot surveil prefix invariant".into());
        }
        // A queued departure can remove the chooser's entire library before
        // this started instruction skips. Replay proves those historical IDs.
        Ok(())
    }

    pub(super) fn begin_surveil(
        &mut self,
        frame: Resolving,
        amount: Quantity,
        source_block: u32,
    ) -> Result<(), String> {
        let player = frame.spell.controller;
        if !self.is_live(player) {
            return self
                .finish_instruction(frame, InstructionOutcome::PlayerActionSkipped { player });
        }
        if amount.is_negative() {
            return Err("negative surveil amount".into());
        }
        if amount == Quantity::zero() {
            return self.finish_instruction(frame, InstructionOutcome::Effect { actual: None });
        }
        let looked = self.library_prefix(player, &amount)?;
        if looked.is_empty() {
            return self.complete_surveil(frame, amount, looked, vec![], vec![], source_block);
        }
        self.state.rules.frame = Frame::ResolutionChoice {
            frame,
            choice: ResolutionChoice::Surveil {
                player,
                looked,
                source_block,
            },
        };
        Ok(())
    }

    pub(super) fn surveil_choice_step(
        &mut self,
        frame: Resolving,
        player: PlayerId,
        looked: Vec<ObjectId>,
        source_block: u32,
    ) -> Result<(), String> {
        self.validate_surveil_choice(&frame, player, &looked, source_block)?;
        if !self.is_live(player) {
            if let Some(input) = self.state.rules.pending.take()
                && (input.player != player || !matches!(input.response, Response::Surveil { .. }))
            {
                return Err("departed surveil input invariant".into());
            }
            return self
                .finish_instruction(frame, InstructionOutcome::PlayerActionSkipped { player });
        }
        let input = self
            .state
            .rules
            .pending
            .take()
            .ok_or("missing surveil input")?;
        let Response::Surveil { top, graveyard } = input.response else {
            return Err("surveil response invariant".into());
        };
        if input.player != player {
            return Err("surveil actor invariant".into());
        }
        self.validate_library_partition(player, &looked, &top, &graveyard)?;
        let Instruction::Surveil { amount, .. } =
            self.program_for(&frame.spell)?.instructions[frame.cursor as usize].clone()
        else {
            return Err("surveil instruction invariant".into());
        };
        self.complete_surveil(frame, amount, looked, top, graveyard, source_block)
    }

    fn complete_surveil(
        &mut self,
        frame: Resolving,
        amount: Quantity,
        looked: Vec<ObjectId>,
        top: Vec<ObjectId>,
        graveyard: Vec<ObjectId>,
        source_block: u32,
    ) -> Result<(), String> {
        let actual = self.commit(
            ProposedEvent::Surveil {
                player: frame.spell.controller,
                amount,
                looked,
                top,
                graveyard,
            },
            BatchCause::Instruction {
                stack_id: frame.spell.id,
                player: frame.spell.controller,
                source: frame.spell.source,
                cursor: frame.cursor,
                source_block,
            },
        )?;
        self.finish_instruction(
            frame,
            InstructionOutcome::Effect {
                actual: Some(actual),
            },
        )
    }

    pub(super) fn apply_surveil(
        &mut self,
        player: PlayerId,
        amount: &Quantity,
        looked: &[ObjectId],
        top: &[ObjectId],
        graveyard: &[ObjectId],
        before: Option<&PreEventFacts>,
    ) -> Result<EventOutcome, String> {
        if amount <= &Quantity::zero() || self.library_prefix(player, amount)? != looked {
            return Err("invalid surveil event prefix".into());
        }
        self.validate_library_partition(player, looked, top, graveyard)?;
        let tail = self
            .state
            .rules
            .libraries
            .get(&player)
            .map(|l| l[looked.len()..].to_vec())
            .unwrap_or_default();
        let mut movements = Vec::with_capacity(graveyard.len());
        for object in graveyard {
            let actual = self.apply_occurrence_with_prior(
                &ProposedEvent::Move {
                    object: *object,
                    destination: Zone::Graveyard { owner: player },
                },
                before,
            )?;
            let EventOutcome::Moved { movement } = actual else {
                return Err("surveil movement invariant".into());
            };
            movements.push(movement);
        }
        if let Some(library) = self.state.rules.libraries.get_mut(&player) {
            *library = top.iter().map(|o| o.card).chain(tail).collect();
        }
        self.remember_surveil(player, top)?;
        for movement in &movements {
            // Newly public successors receive handles in the chosen public movement order.
            self.capture_trigger_source_handles(movement.successor)?;
        }
        Ok(EventOutcome::Surveilled {
            player,
            looked: looked.to_vec(),
            top: top.to_vec(),
            graveyard: graveyard.to_vec(),
            movements,
        })
    }
}
