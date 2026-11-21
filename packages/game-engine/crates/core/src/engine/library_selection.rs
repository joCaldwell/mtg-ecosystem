//! CR121.5/400.7/404.3/608.2c/903.9b: selecting into hand is not drawing.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
impl Game {
    pub(super) fn selection_destination(
        &self,
        frame: &Resolving,
    ) -> Result<LibraryRemainder, String> {
        match self
            .program_for(self.entry(frame.stack_id)?)?
            .instructions
            .get(frame.cursor as usize)
        {
            Some(Instruction::SelectLibraryCard { remainder, .. }) => Ok(*remainder),
            _ => Err("selection instruction invariant".into()),
        }
    }
    pub(super) fn validate_selection_choice(
        &self,
        frame: &Resolving,
        player: PlayerId,
        looked: &[ObjectId],
        source_block: u32,
    ) -> Result<(), String> {
        let Some(Instruction::SelectLibraryCard {
            amount,
            source_block: expected,
            ..
        }) = self
            .program_for(self.entry(frame.stack_id)?)?
            .instructions
            .get(frame.cursor as usize)
        else {
            return Err("selection source binding invariant".into());
        };
        if player != self.entry(frame.stack_id)?.controller
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
            return Err("selection prefix invariant".into());
        }
        Ok(())
    }
    pub(super) fn validate_selection_partition(
        &self,
        player: PlayerId,
        looked: &[ObjectId],
        hand: &[ObjectId],
        remainder: &[ObjectId],
    ) -> Result<(), String> {
        self.validate_library_partition(player, looked, hand, remainder)?;
        if hand.len() != usize::from(!looked.is_empty()) {
            return Err("selection requires one available card".into());
        }
        Ok(())
    }
    pub(super) fn begin_library_selection(
        &mut self,
        frame: Resolving,
        amount: Quantity,
        source_block: u32,
    ) -> Result<(), String> {
        let player = self.entry(frame.stack_id)?.controller;
        if !self.is_live(player) {
            return self
                .finish_instruction(frame, InstructionOutcome::PlayerActionSkipped { player });
        }
        if amount.is_negative() {
            return Err("negative selection count".into());
        }
        if amount == Quantity::zero() {
            return self.finish_instruction(frame, InstructionOutcome::Effect { actual: None });
        }
        let looked = self.library_prefix(player, &amount)?;
        if looked.is_empty() {
            return self.complete_library_selection(
                frame,
                looked,
                vec![],
                vec![],
                source_block,
                false,
            );
        }
        self.state.rules.frame = Frame::ResolutionChoice {
            frame,
            choice: ResolutionChoice::LibrarySelect {
                player,
                looked,
                source_block,
            },
        };
        Ok(())
    }
    pub(super) fn library_selection_step(
        &mut self,
        frame: Resolving,
        player: PlayerId,
        looked: Vec<ObjectId>,
        source_block: u32,
    ) -> Result<(), String> {
        self.validate_selection_choice(&frame, player, &looked, source_block)?;
        if !self.is_live(player) {
            if let Some(input) = self.state.rules.pending.take()
                && (input.player != player
                    || !matches!(input.response, Response::LibrarySelect { .. }))
            {
                return Err("departed selection input invariant".into());
            }
            return self
                .finish_instruction(frame, InstructionOutcome::PlayerActionSkipped { player });
        }
        let input = self
            .state
            .rules
            .pending
            .take()
            .ok_or("missing library selection")?;
        let Response::LibrarySelect { hand, remainder } = input.response else {
            return Err("selection input kind".into());
        };
        if input.player != player {
            return Err("selection input actor".into());
        }
        self.validate_selection_partition(player, &looked, &hand, &remainder)?;
        if let Some(object) = hand.first().copied()
            && self.state.rules.commanders.history(object.card).is_some()
        {
            self.state.rules.frame = Frame::CommanderReturn {
                object,
                destination: Zone::Hand { owner: player },
                continuation: Box::new(CommanderContinuation::LibrarySelect {
                    frame,
                    looked,
                    hand,
                    remainder,
                    source_block,
                }),
            };
            return Ok(());
        }
        self.complete_library_selection(frame, looked, hand, remainder, source_block, false)
    }
    pub(super) fn complete_library_selection(
        &mut self,
        frame: Resolving,
        looked: Vec<ObjectId>,
        hand: Vec<ObjectId>,
        remainder: Vec<ObjectId>,
        source_block: u32,
        to_command: bool,
    ) -> Result<(), String> {
        let player = self.entry(frame.stack_id)?.controller;
        if !self.is_live(player) {
            return self
                .finish_instruction(frame, InstructionOutcome::PlayerActionSkipped { player });
        }
        let Some(Instruction::SelectLibraryCard {
            amount,
            remainder: destination,
            source_block: expected,
        }) = self
            .program_for(self.entry(frame.stack_id)?)?
            .instructions
            .get(frame.cursor as usize)
            .cloned()
        else {
            return Err("selection completion source".into());
        };
        if expected != source_block {
            return Err("selection completion block".into());
        }
        self.complete_library_effect(
            frame,
            ProposedEvent::LibrarySelect {
                player,
                amount,
                destination,
                looked,
                hand,
                remainder,
                hand_to_command: to_command,
            },
            source_block,
        )
    }
    pub(super) fn apply_library_selection(
        &mut self,
        proposed: &ProposedEvent,
        before: Option<&PreEventFacts>,
        replacement: Option<&BTreeMap<ObjectId, Zone>>,
    ) -> Result<EventOutcome, String> {
        let ProposedEvent::LibrarySelect {
            player,
            amount,
            destination,
            looked,
            hand,
            remainder,
            hand_to_command,
        } = proposed
        else {
            return Err("selection event kind".into());
        };
        let player = *player;
        let destination = *destination;
        let to_command = *hand_to_command;
        if amount <= &Quantity::zero()
            || self.library_prefix(player, amount)?.as_slice() != looked.as_slice()
        {
            return Err("invalid selection prefix".into());
        }
        self.validate_selection_partition(player, looked, hand, remainder)?;
        if to_command
            && !hand
                .first()
                .is_some_and(|o| self.state.rules.commanders.history(o.card).is_some())
        {
            return Err("invalid selected commander replacement".into());
        }
        let tail = self
            .state
            .rules
            .libraries
            .get(&player)
            .into_iter()
            .flatten()
            .skip(looked.len())
            .copied()
            .collect::<Vec<_>>();
        let chosen = if let Some(object) = hand.first() {
            let actual = self.apply_occurrence_with_prior(
                &ProposedEvent::Move {
                    object: *object,
                    destination: if to_command {
                        Zone::Command { owner: player }
                    } else {
                        Zone::Hand { owner: player }
                    },
                },
                before,
            )?;
            let EventOutcome::Moved { movement } = actual else {
                return Err("selection movement invariant".into());
            };
            Some(movement)
        } else {
            None
        };
        let graveyard = match destination {
            LibraryRemainder::Graveyard => {
                let m =
                    self.move_library_cards_to_graveyard(player, remainder, before, replacement)?;
                self.remember_library_selection(player, &[])?;
                m
            }
            LibraryRemainder::Bottom => {
                let mut library = tail;
                library.extend(remainder.iter().map(|o| o.card));
                self.state.rules.libraries.insert(player, library);
                self.remember_library_selection(player, remainder)?;
                vec![]
            }
        };
        Ok(EventOutcome::LibrarySelected {
            player,
            hand: chosen,
            remainder: remainder.to_vec(),
            graveyard,
        })
    }
}
