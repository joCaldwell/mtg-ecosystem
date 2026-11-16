//! CR 109.5, 400.7, 608.2c/h, 701.22a-d. Looking and same-zone ordering
//! never create successor incarnations or a priority window.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

impl Game {
    pub(super) fn library_looked(&self, viewer: PlayerId) -> Option<&[ObjectId]> {
        match &self.state.rules.frame {
            Frame::ResolutionChoice {
                choice:
                    ResolutionChoice::Scry { player, looked, .. }
                    | ResolutionChoice::Surveil { player, looked, .. },
                ..
            } if *player == viewer && self.is_live(viewer) => Some(looked),
            _ => None,
        }
    }
    pub(super) fn library_prefix(
        &self,
        player: PlayerId,
        amount: &Quantity,
    ) -> Result<Vec<ObjectId>, String> {
        if amount.is_negative() {
            return Err("negative scry amount".into());
        }
        let mut looked = vec![];
        for (index, card) in self
            .state
            .rules
            .libraries
            .get(&player)
            .into_iter()
            .flatten()
            .enumerate()
        {
            if Quantity::from(i64::try_from(index).map_err(|_| "scry index limit")?) >= *amount {
                break;
            }
            let object = self.object_id(*card).ok_or("missing scry library card")?;
            if self.current(object)?.zone != (Zone::Library { owner: player }) {
                return Err("scry library zone invariant".into());
            }
            looked.push(object);
        }
        Ok(looked)
    }

    pub(super) fn validate_library_partition(
        &self,
        player: PlayerId,
        looked: &[ObjectId],
        top: &[ObjectId],
        bottom: &[ObjectId],
    ) -> Result<(), String> {
        if !self.is_live(player)
            || top.len().checked_add(bottom.len()) != Some(looked.len())
            || top.iter().chain(bottom).copied().collect::<BTreeSet<_>>()
                != looked.iter().copied().collect::<BTreeSet<_>>()
            || looked.iter().collect::<BTreeSet<_>>().len() != looked.len()
        {
            return Err("invalid scry partition".into());
        }
        Ok(())
    }

    pub(super) fn validate_scry_choice(
        &self,
        frame: &Resolving,
        player: PlayerId,
        looked: &[ObjectId],
        source_block: u32,
    ) -> Result<(), String> {
        let program = self.program_for(&frame.spell)?;
        let Some(Instruction::Scry {
            amount,
            source_block: expected,
        }) = program.instructions.get(frame.cursor as usize)
        else {
            return Err("snapshot scry instruction binding".into());
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
            return Err("snapshot scry prefix invariant".into());
        }
        // A queued departure can remove the chooser's entire library before
        // this started instruction skips. Replay proves those historical IDs.
        Ok(())
    }

    pub(super) fn begin_scry(
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
            return Err("negative scry amount".into());
        }
        if amount == Quantity::zero() {
            return self.finish_instruction(frame, InstructionOutcome::Effect { actual: None });
        }
        let looked = self.library_prefix(player, &amount)?;
        if looked.is_empty() {
            return self.complete_scry(frame, amount, looked, vec![], vec![], source_block);
        }
        self.state.rules.frame = Frame::ResolutionChoice {
            frame,
            choice: ResolutionChoice::Scry {
                player,
                looked,
                source_block,
            },
        };
        Ok(())
    }

    pub(super) fn scry_choice_step(
        &mut self,
        frame: Resolving,
        player: PlayerId,
        looked: Vec<ObjectId>,
        source_block: u32,
    ) -> Result<(), String> {
        self.validate_scry_choice(&frame, player, &looked, source_block)?;
        if !self.is_live(player) {
            if let Some(input) = self.state.rules.pending.take()
                && (input.player != player || !matches!(input.response, Response::Scry { .. }))
            {
                return Err("departed scry input invariant".into());
            }
            return self
                .finish_instruction(frame, InstructionOutcome::PlayerActionSkipped { player });
        }
        let input = self
            .state
            .rules
            .pending
            .take()
            .ok_or("missing scry input")?;
        let Response::Scry { top, bottom } = input.response else {
            return Err("scry response invariant".into());
        };
        if input.player != player {
            return Err("scry actor invariant".into());
        }
        self.validate_library_partition(player, &looked, &top, &bottom)?;
        let Instruction::Scry { amount, .. } =
            self.program_for(&frame.spell)?.instructions[frame.cursor as usize].clone()
        else {
            return Err("scry instruction invariant".into());
        };
        self.complete_scry(frame, amount, looked, top, bottom, source_block)
    }

    fn complete_scry(
        &mut self,
        frame: Resolving,
        amount: Quantity,
        looked: Vec<ObjectId>,
        top: Vec<ObjectId>,
        bottom: Vec<ObjectId>,
        source_block: u32,
    ) -> Result<(), String> {
        let actual = self.commit(
            ProposedEvent::Scry {
                player: frame.spell.controller,
                amount,
                looked,
                top,
                bottom,
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

    pub(super) fn apply_scry(
        &mut self,
        player: PlayerId,
        amount: &Quantity,
        looked: &[ObjectId],
        top: &[ObjectId],
        bottom: &[ObjectId],
    ) -> Result<EventOutcome, String> {
        if amount <= &Quantity::zero() || self.library_prefix(player, amount)? != looked {
            return Err("invalid scry event prefix".into());
        }
        self.validate_library_partition(player, looked, top, bottom)?;
        if let Some(library) = self.state.rules.libraries.get_mut(&player) {
            *library = top
                .iter()
                .map(|o| o.card)
                .chain(library[looked.len()..].iter().copied())
                .chain(bottom.iter().map(|o| o.card))
                .collect();
        }
        self.remember_scry(player, top, bottom)?;
        Ok(EventOutcome::Scried {
            player,
            looked: looked.to_vec(),
            top: top.to_vec(),
            bottom: bottom.to_vec(),
        })
    }

    pub(super) fn library_look_visible_to(&self, viewer: PlayerId, object: ObjectId) -> bool {
        self.library_looked(viewer)
            .is_some_and(|looked| looked.contains(&object))
    }
}
