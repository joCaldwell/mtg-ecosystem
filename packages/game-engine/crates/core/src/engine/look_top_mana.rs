//! CR 109.3, 202.2, 107.4e/f, 121.5, 608.2c and 701.20a.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use mtg_contracts::{MANA_COMBINATION_COLORS, validate_mana_combination};

impl Game {
    fn look_top_mana_spec(&self, frame: &Resolving) -> Result<(Quantity, Quantity, u32), String> {
        match self
            .program_for(&frame.spell)?
            .instructions
            .get(frame.cursor as usize)
        {
            Some(Instruction::LookTopMayRevealForMana {
                minimum_symbols,
                mana,
                source_block,
            }) => Ok((minimum_symbols.clone(), mana.clone(), *source_block)),
            _ => Err("missing top card mana instruction".into()),
        }
    }

    fn look_top_mana_eligible(&self, frame: &Resolving, object: ObjectId) -> Result<bool, String> {
        let (minimum_symbols, _, _) = self.look_top_mana_spec(frame)?;
        Ok(self
            .definition(object.card)?
            .characteristics
            .colored_mana_symbol_count()
            >= minimum_symbols)
    }

    fn look_top_mana_colors(&self, object: ObjectId) -> Result<Vec<Color>, String> {
        let card = &self.definition(object.card)?.characteristics;
        Ok(MANA_COMBINATION_COLORS
            .into_iter()
            .filter(|color| card.colors.contains(color))
            .collect())
    }

    fn look_top_mana_cause(&self, frame: &Resolving) -> Result<BatchCause, String> {
        let (_, _, source_block) = self.look_top_mana_spec(frame)?;
        Ok(BatchCause::Instruction {
            stack_id: frame.stack_id,
            player: frame.spell.controller,
            source: frame.spell.source,
            cursor: frame.cursor,
            source_block,
        })
    }

    pub(super) fn begin_look_top_mana(&mut self, frame: Resolving) -> Result<(), String> {
        let player = frame.spell.controller;
        if !self.is_live(player) {
            return self
                .finish_instruction(frame, InstructionOutcome::PlayerActionSkipped { player });
        }
        let looked = self.library_prefix(player, &1.into())?;
        let Some(&object) = looked.first() else {
            return self.finish_instruction(frame, InstructionOutcome::Effect { actual: None });
        };
        if !self.look_top_mana_eligible(&frame, object)? {
            return self.move_look_top_mana(frame, object, false);
        }
        self.state.rules.frame = Frame::ResolutionChoice {
            frame,
            choice: ResolutionChoice::LookTopMana {
                player,
                looked,
                revealed: false,
            },
        };
        Ok(())
    }

    pub(super) fn validate_look_top_mana_choice(
        &self,
        frame: &Resolving,
        player: PlayerId,
        looked: &[ObjectId],
        revealed: bool,
    ) -> Result<(), String> {
        let [object] = looked else {
            return Err("look requires exactly one card".into());
        };
        if player != frame.spell.controller
            || !self.look_top_mana_eligible(frame, *object)?
            || self.is_live(player)
                && (self.library_prefix(player, &1.into())? != looked
                    || revealed
                        && (self.look_top_mana_colors(*object)?.is_empty()
                            || !self.state.knowledge.visibility[player.0 as usize]
                                .public_tucked_cards
                                .contains(object)))
        {
            return Err("invalid retained top card mana choice".into());
        }
        Ok(())
    }

    pub(super) fn look_top_mana_request(
        &self,
        frame: &Resolving,
        player: PlayerId,
        looked: &[ObjectId],
        revealed: bool,
    ) -> Result<Request, String> {
        self.validate_look_top_mana_choice(frame, player, looked, revealed)?;
        let decision_id = self.state.rules.revision;
        if !revealed {
            return Ok(Request::OptionalEffect {
                player,
                decision_id,
            });
        }
        let (_, total, _) = self.look_top_mana_spec(frame)?;
        Ok(Request::ManaCombination {
            player,
            decision_id,
            total,
            colors: self.look_top_mana_colors(looked[0])?,
        })
    }

    pub(super) fn validate_look_top_mana_response(
        &self,
        player: PlayerId,
        response: &Response,
    ) -> Option<Result<(), String>> {
        let Frame::ResolutionChoice {
            frame,
            choice:
                ResolutionChoice::LookTopMana {
                    player: chooser,
                    looked,
                    revealed,
                },
        } = &self.state.rules.frame
        else {
            return None;
        };
        Some((|| {
            if player != *chooser || !self.is_live(player) {
                return Err("wrong top card mana chooser".into());
            }
            match (
                self.look_top_mana_request(frame, player, looked, *revealed)?,
                response,
            ) {
                (Request::OptionalEffect { .. }, Response::OptionalEffect { .. }) => Ok(()),
                (
                    Request::ManaCombination { colors, total, .. },
                    Response::ManaCombination { amounts },
                ) => validate_mana_combination(&colors, &total, amounts),
                _ => Err("wrong top card response".into()),
            }
        })())
    }

    pub(super) fn look_top_mana_step(
        &mut self,
        frame: Resolving,
        player: PlayerId,
        looked: Vec<ObjectId>,
        revealed: bool,
    ) -> Result<(), String> {
        self.validate_look_top_mana_choice(&frame, player, &looked, revealed)?;
        let input = self.state.rules.pending.take();
        if !self.is_live(player) {
            return self
                .finish_instruction(frame, InstructionOutcome::PlayerActionSkipped { player });
        }
        let input = input.ok_or("missing top card decision")?;
        self.validate_look_top_mana_response(input.player, &input.response)
            .ok_or("missing top card response owner")??;
        let object = looked[0];
        match input.response {
            Response::OptionalEffect { perform: false } => {
                self.move_look_top_mana(frame, object, false)
            }
            Response::OptionalEffect { perform: true } => {
                self.commit(
                    ProposedEvent::RevealLibraryCard { player, object },
                    self.look_top_mana_cause(&frame)?,
                )?;
                if self.look_top_mana_colors(object)?.is_empty() {
                    return self.move_look_top_mana(frame, object, true);
                }
                self.state.rules.frame = Frame::ResolutionChoice {
                    frame,
                    choice: ResolutionChoice::LookTopMana {
                        player,
                        looked,
                        revealed: true,
                    },
                };
                Ok(())
            }
            Response::ManaCombination { amounts } => {
                let events = MANA_COMBINATION_COLORS
                    .into_iter()
                    .zip(amounts)
                    .filter(|(_, amount)| *amount != Quantity::zero())
                    .map(|(color, amount)| EventGroup::Occurrence {
                        proposed: ProposedEvent::AddMana {
                            restriction: None,
                            player,
                            color,
                            amount,
                            source: frame.spell.source,
                        },
                    })
                    .collect();
                self.commit_batch(
                    self.look_top_mana_cause(&frame)?,
                    EventGroup::Ordered { children: events },
                )?;
                self.move_look_top_mana(frame, object, true)
            }
            _ => Err("unexpected top card mana action".into()),
        }
    }

    pub(super) fn look_top_mana_reveal_authorized(
        &self,
        player: PlayerId,
        object: ObjectId,
    ) -> bool {
        matches!(&self.state.rules.frame, Frame::ResolutionChoice {
            frame, choice: ResolutionChoice::LookTopMana { player: chooser, looked, revealed: false },
        } if *chooser == player && looked.as_slice() == [object]
            && self.validate_look_top_mana_choice(frame, player, looked, false).is_ok())
    }

    pub(super) fn retained_look_top_mana_source(&self, player: PlayerId, source: ObjectId) -> bool {
        matches!(&self.state.rules.frame, Frame::ResolutionChoice {
            frame, choice: ResolutionChoice::LookTopMana { player: chooser, revealed: true, .. },
        } if *chooser == player && frame.spell.source == source)
    }

    fn move_look_top_mana(
        &mut self,
        frame: Resolving,
        object: ObjectId,
        revealed: bool,
    ) -> Result<(), String> {
        let player = frame.spell.controller;
        if self.state.rules.commanders.history(object.card).is_some() {
            self.state.rules.frame = Frame::CommanderReturn {
                object,
                destination: Zone::Hand { owner: player },
                continuation: Box::new(CommanderContinuation::LookTopMana { frame, revealed }),
            };
            return Ok(());
        }
        self.finish_look_top_mana(frame, object, revealed, false)
    }

    pub(super) fn validate_look_top_mana_move(
        &self,
        frame: &Resolving,
        object: ObjectId,
        revealed: bool,
    ) -> Result<(), String> {
        self.look_top_mana_spec(frame)?;
        let player = frame.spell.controller;
        if self.is_live(player)
            && (self.library_prefix(player, &1.into())? != [object]
                || revealed
                    && (!self.look_top_mana_eligible(frame, object)?
                        || !self.state.knowledge.visibility[player.0 as usize]
                            .public_tucked_cards
                            .contains(&object)))
        {
            return Err("invalid top card placement".into());
        }
        Ok(())
    }

    pub(super) fn finish_look_top_mana(
        &mut self,
        frame: Resolving,
        object: ObjectId,
        revealed: bool,
        to_command: bool,
    ) -> Result<(), String> {
        let player = frame.spell.controller;
        if !self.is_live(player) {
            return self
                .finish_instruction(frame, InstructionOutcome::PlayerActionSkipped { player });
        }
        self.validate_look_top_mana_move(&frame, object, revealed)?;
        let informed = self
            .state
            .knowledge
            .visibility
            .iter()
            .map(|view| view.public_tucked_cards.contains(&object))
            .collect::<Vec<_>>();
        let actual = self.commit(
            ProposedEvent::Move {
                object,
                destination: if to_command {
                    Zone::Command { owner: player }
                } else {
                    Zone::Hand { owner: player }
                },
            },
            self.look_top_mana_cause(&frame)?,
        )?;
        if !to_command {
            let EventOutcome::Moved { movement } = &actual else {
                return Err("missing looked card hand move".into());
            };
            for (knowledge, knew_top) in self.state.knowledge.visibility.iter_mut().zip(informed) {
                if knew_top {
                    knowledge.revealed_hand.insert(movement.successor);
                }
            }
        }
        self.finish_instruction(
            frame,
            InstructionOutcome::Effect {
                actual: Some(actual),
            },
        )
    }
}
