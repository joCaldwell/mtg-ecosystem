//! CR401.4/608.2c/701.24: private ordering and optional seeded shuffle.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
impl Game {
    pub(super) fn validate_library_order_choice(
        &self,
        frame: &Resolving,
        player: PlayerId,
        looked: &[ObjectId],
        block: u32,
    ) -> Result<(), String> {
        let Some(Instruction::OrderLibraryTop {
            amount,
            source_block,
        }) = self
            .program_for(&frame.spell)?
            .instructions
            .get(frame.cursor as usize)
        else {
            return Err("library order source invariant".into());
        };
        if player != frame.spell.controller
            || *source_block != block
            || amount <= &Quantity::zero()
            || looked.is_empty()
            || looked.len() > self.state.rules.cards.len()
            || looked.iter().collect::<BTreeSet<_>>().len() != looked.len()
            || looked
                .iter()
                .any(|o| !self.state.rules.cards.contains_key(&o.card))
            || (self.is_live(player) && self.library_prefix(player, amount)? != looked)
        {
            return Err("library order prefix invariant".into());
        }
        Ok(())
    }
    pub(super) fn begin_library_order(
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
            return Err("negative library order count".into());
        }
        if amount == Quantity::zero() {
            return self.finish_instruction(frame, InstructionOutcome::Effect { actual: None });
        }
        let looked = self.library_prefix(player, &amount)?;
        if looked.is_empty() {
            return self.complete_library_order(frame, amount, looked, vec![], source_block);
        }
        self.state.rules.frame = Frame::ResolutionChoice {
            frame,
            choice: ResolutionChoice::LibraryOrder {
                player,
                looked,
                source_block,
            },
        };
        Ok(())
    }
    pub(super) fn library_order_step(
        &mut self,
        frame: Resolving,
        player: PlayerId,
        looked: Vec<ObjectId>,
        source_block: u32,
    ) -> Result<(), String> {
        self.validate_library_order_choice(&frame, player, &looked, source_block)?;
        if !self.is_live(player) {
            if let Some(input) = self.state.rules.pending.take()
                && (input.player != player
                    || !matches!(input.response, Response::LibraryOrder { .. }))
            {
                return Err("departed library ordering input invariant".into());
            }
            return self
                .finish_instruction(frame, InstructionOutcome::PlayerActionSkipped { player });
        }
        let input = self
            .state
            .rules
            .pending
            .take()
            .ok_or("missing library ordering input")?;
        let Response::LibraryOrder { objects } = input.response else {
            return Err("library ordering response kind".into());
        };
        if input.player != player {
            return Err("library ordering actor".into());
        }
        self.validate_library_partition(player, &looked, &objects, &[])?;
        let Instruction::OrderLibraryTop { amount, .. } =
            self.program_for(&frame.spell)?.instructions[frame.cursor as usize].clone()
        else {
            return Err("library order source".into());
        };
        self.complete_library_order(frame, amount, looked, objects, source_block)
    }
    fn complete_library_order(
        &mut self,
        frame: Resolving,
        amount: Quantity,
        looked: Vec<ObjectId>,
        order: Vec<ObjectId>,
        source_block: u32,
    ) -> Result<(), String> {
        let actual = self.commit(
            ProposedEvent::OrderLibraryTop {
                player: frame.spell.controller,
                amount,
                looked,
                order,
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
    pub(super) fn apply_library_order(
        &mut self,
        player: PlayerId,
        amount: &Quantity,
        looked: &[ObjectId],
        order: &[ObjectId],
    ) -> Result<EventOutcome, String> {
        if amount <= &Quantity::zero() || self.library_prefix(player, amount)? != looked {
            return Err("invalid library order event prefix".into());
        }
        self.reorder_library_prefix(player, looked, order, &[])?;
        self.remember_library_order(player, order)?;
        Ok(EventOutcome::LibraryOrdered {
            player,
            looked: looked.to_vec(),
            order: order.to_vec(),
        })
    }
    pub(super) fn begin_optional_shuffle(
        &mut self,
        frame: Resolving,
        source_block: u32,
    ) -> Result<(), String> {
        let player = frame.spell.controller;
        if !self.is_live(player) {
            return self
                .finish_instruction(frame, InstructionOutcome::PlayerActionSkipped { player });
        }
        self.state.rules.frame = Frame::ResolutionChoice {
            frame,
            choice: ResolutionChoice::OptionalShuffle {
                player,
                source_block,
            },
        };
        Ok(())
    }
    pub(super) fn optional_shuffle_step(
        &mut self,
        frame: Resolving,
        player: PlayerId,
        source_block: u32,
    ) -> Result<(), String> {
        if frame.spell.controller != player
            || !matches!(self.program_for(&frame.spell)?.instructions.get(frame.cursor as usize),Some(Instruction::OptionalShuffleLibrary{source_block:b})if *b==source_block)
        {
            return Err("optional shuffle source invariant".into());
        }
        if !self.is_live(player) {
            if let Some(input) = self.state.rules.pending.take()
                && (input.player != player
                    || !matches!(input.response, Response::OptionalEffect { .. }))
            {
                return Err("departed optional shuffle input invariant".into());
            }
            return self
                .finish_instruction(frame, InstructionOutcome::PlayerActionSkipped { player });
        }
        let input = self
            .state
            .rules
            .pending
            .take()
            .ok_or("missing optional shuffle input")?;
        let Response::OptionalEffect { perform } = input.response else {
            return Err("optional shuffle response kind".into());
        };
        if input.player != player {
            return Err("optional shuffle actor".into());
        }
        if !perform {
            return self.finish_instruction(frame, InstructionOutcome::Effect { actual: None });
        }
        self.begin_player_library_shuffle(frame, player, source_block)
    }

    /// After any preceding zone movement, derive the shuffle range from the
    /// actual library. The current instruction owns the player and source block.
    pub(super) fn begin_player_library_shuffle(
        &mut self,
        frame: Resolving,
        player: PlayerId,
        source_block: u32,
    ) -> Result<(), String> {
        let index = u32::try_from(
            self.state
                .rules
                .libraries
                .get(&player)
                .map_or(0, |l| l.len().saturating_sub(1)),
        )
        .map_err(|_| "shuffle length resource limit")?;
        self.state.rules.frame = Frame::ResolutionChoice {
            frame,
            choice: ResolutionChoice::Shuffle {
                player,
                index,
                source_block,
            },
        };
        Ok(())
    }
}
