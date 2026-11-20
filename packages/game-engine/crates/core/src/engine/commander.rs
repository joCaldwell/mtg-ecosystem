use super::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "continuation", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum CommanderContinuation {
    CascadeBottom {
        progress: Box<CascadeProgress>,
    },
    GraveyardShuffle {
        frame: Resolving,
        objects: Vec<ObjectId>,
        remaining: Vec<ObjectId>,
        choices: Vec<(ObjectId, bool)>,
        source_block: u32,
    },
    HandReturn {
        frame: Resolving,
        objects: Vec<ObjectId>,
        remaining: Vec<ObjectId>,
        choices: Vec<(ObjectId, bool)>,
        source_block: u32,
    },
    LookTopMana {
        frame: Resolving,
        revealed: bool,
    },
    LibrarySelect {
        frame: Resolving,
        looked: Vec<ObjectId>,
        hand: Vec<ObjectId>,
        remainder: Vec<ObjectId>,
        source_block: u32,
    },
    BoardReturn {
        frame: Resolving,
        objects: Vec<ObjectId>,
        remaining: Vec<ObjectId>,
        choices: Vec<(ObjectId, bool)>,
        source_block: u32,
    },
    SpellMove {
        frame: Resolving,
        source_block: u32,
    },
    ResolutionDraw {
        frame: Resolving,
        player: PlayerId,
        source_block: u32,
    },
    Draw {
        player: PlayerId,
    },
    StateBased {
        resume: Frame,
    },
    StateBasedBatch {
        remaining: Vec<ObjectId>,
        choices: Vec<(ObjectId, bool)>,
        resume: Frame,
    },
}

impl Game {
    pub(super) fn previous_commander_choices(&self) -> Vec<(ObjectId, bool)> {
        match &self.state.rules.frame {
            Frame::GraveyardGroup { tx } => tx.prior_choices(self),
            Frame::GraveyardReplacement { transaction } => transaction.prior_choices(),
            Frame::CommanderReturn { continuation, .. } => match continuation.as_ref() {
                CommanderContinuation::GraveyardShuffle { choices, .. }
                | CommanderContinuation::StateBasedBatch { choices, .. }
                | CommanderContinuation::BoardReturn { choices, .. } => choices.clone(),
                _ => vec![],
            },
            _ => vec![],
        }
    }
    pub(super) fn visible_commander_choices(
        &self,
        viewer: PlayerId,
    ) -> Result<Vec<VisibleCommanderReturnChoice>, String> {
        self.previous_commander_choices()
            .into_iter()
            .map(|(id, to_command)| {
                Ok(VisibleCommanderReturnChoice {
                    designation: self.commander_designation(viewer, id.card)?,
                    owner: self
                        .state
                        .rules
                        .cards
                        .get(&id.card)
                        .ok_or("missing commander choice card")?
                        .owner,
                    to_command,
                })
            })
            .collect()
    }

    pub(super) fn commander_designation(
        &self,
        viewer: PlayerId,
        card: CardId,
    ) -> Result<String, String> {
        self.state
            .rules
            .commanders
            .records
            .keys()
            .position(|id| *id == card)
            .map(|index| format!("v{}-c{index}", viewer.0))
            .ok_or_else(|| "physical card is not designated".into())
    }
    // CR 903.8 / 601.2f: use completed prior command-origin casts of this
    // physical commander by this caster. Pinned official rules:
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    pub(super) fn casting_cost(
        &self,
        card: CardId,
        caster: PlayerId,
        from: Zone,
    ) -> Result<ManaCost, String> {
        self.announcement_cost(card, caster, from, None)
    }
    pub(super) fn announcement_cost(
        &self,
        card: CardId,
        caster: PlayerId,
        from: Zone,
        alternative: Option<cascade::AlternativeCost>,
    ) -> Result<ManaCost, String> {
        self.announcement_cost_with_additional(card, caster, from, alternative, &[])
    }
    pub(super) fn announcement_cost_with_additional(
        &self,
        card: CardId,
        caster: PlayerId,
        from: Zone,
        alternative: Option<cascade::AlternativeCost>,
        selected: &[String],
    ) -> Result<ManaCost, String> {
        let lock = self.cost_lock_inputs(card, caster, from)?;
        self.construct_spell_cost(card, alternative, None, selected, None, &lock)
    }

    // CR 903.9a / 704.3: consume a fresh graveyard/exile incarnation's next
    // SBA opportunity even if declined. Historical pools permit at most one.
    // Match pods preserve every simultaneous fresh opportunity and process
    // owner choices APNAP before one simultaneous move wave and trigger placement.
    pub(super) fn offer_commander_sba(&mut self, resume: Frame) -> Result<bool, String> {
        let eligible = self
            .state
            .rules
            .commander_returns
            .iter()
            .copied()
            .find(|id| {
                self.current(*id)
                    .is_ok_and(|o| matches!(o.zone, Zone::Graveyard { .. } | Zone::Exile))
                    && self.is_live(self.state.rules.cards[&id.card].owner)
            });
        if self.state.rules.commander_returns.len() > 1 {
            let mut objects = self
                .state
                .rules
                .commander_returns
                .iter()
                .copied()
                .filter(|id| {
                    self.current(*id)
                        .is_ok_and(|o| matches!(o.zone, Zone::Graveyard { .. } | Zone::Exile))
                        && self.is_live(self.state.rules.cards[&id.card].owner)
                })
                .collect::<Vec<_>>();
            objects.sort_by_key(|id| {
                (
                    (self.state.rules.cards[&id.card].owner.0 + 4 - self.state.rules.turn_owner.0)
                        % 4,
                    *id,
                )
            });
            self.state.rules.commander_returns.clear();
            if !objects.is_empty() {
                let object = objects.remove(0);
                self.state.rules.frame = Frame::CommanderReturn {
                    object,
                    destination: self.current(object)?.zone,
                    continuation: Box::new(CommanderContinuation::StateBasedBatch {
                        remaining: objects,
                        choices: vec![],
                        resume,
                    }),
                };
                return Ok(true);
            }
            return Ok(false);
        }
        self.state.rules.commander_returns.clear();
        if let Some(object) = eligible {
            self.state.rules.frame = Frame::CommanderReturn {
                object,
                destination: self.current(object)?.zone,
                continuation: Box::new(CommanderContinuation::StateBased { resume }),
            };
            Ok(true)
        } else {
            Ok(false)
        }
    }
    // CR 903.9b applies from any zone, including drawing a tucked commander.
    // No library row becomes visible and no hidden object handle is minted.
    pub(super) fn offer_commander_draw(
        &mut self,
        player: PlayerId,
        card: CardId,
    ) -> Result<bool, String> {
        if self.state.rules.commanders.history(card).is_none() {
            return Ok(false);
        }
        let object = self
            .object_id(card)
            .ok_or("missing commander draw source")?;
        if self.current(object)?.zone != (Zone::Library { owner: player }) {
            return Err("commander draw source invariant".into());
        }
        self.state.rules.frame = Frame::CommanderReturn {
            object,
            destination: Zone::Hand { owner: player },
            continuation: Box::new(CommanderContinuation::Draw { player }),
        };
        Ok(true)
    }
    pub(super) fn answer_commander_return(
        &mut self,
        object: ObjectId,
        destination: Zone,
        continuation: CommanderContinuation,
    ) -> Result<(), String> {
        let input = self
            .state
            .rules
            .pending
            .take()
            .ok_or("missing commander return choice")?;
        let Response::CommanderReturn { to_command } = input.response else {
            return Err("commander return response invariant".into());
        };
        if input.player != self.state.rules.cards[&object.card].owner {
            return Err("commander return owner invariant".into());
        }
        self.resume_commander_return(object, destination, continuation, to_command)
    }
    fn resume_commander_return(
        &mut self,
        object: ObjectId,
        destination: Zone,
        continuation: CommanderContinuation,
        to_command: bool,
    ) -> Result<(), String> {
        let owner = self.state.rules.cards[&object.card].owner;
        let present = self.current(object).is_ok() && self.is_live(owner);
        let actual_destination = if to_command {
            Zone::Command { owner }
        } else {
            destination
        };
        match continuation {
            CommanderContinuation::CascadeBottom { progress } => {
                self.resume_cascade_commander(progress, object, to_command)?
            }
            CommanderContinuation::GraveyardShuffle {
                frame,
                objects,
                mut remaining,
                mut choices,
                source_block,
            } => {
                if remaining.first() != Some(&object) {
                    return Err("graveyard shuffle commander order".into());
                }
                remaining.remove(0);
                if present {
                    choices.push((object, to_command));
                }
                self.continue_graveyard_shuffle(frame, objects, remaining, choices, source_block)?;
            }
            CommanderContinuation::HandReturn {
                frame,
                objects,
                mut remaining,
                mut choices,
                source_block,
            } => {
                if remaining.first() != Some(&object) {
                    return Err("hand return commander order".into());
                }
                remaining.remove(0);
                if present {
                    choices.push((object, to_command));
                }
                self.continue_hand_return(frame, objects, remaining, choices, source_block)?;
            }
            CommanderContinuation::LookTopMana { frame, revealed } => {
                self.finish_look_top_mana(frame, object, revealed, to_command)?;
            }
            CommanderContinuation::LibrarySelect {
                frame,
                looked,
                hand,
                remainder,
                source_block,
            } => {
                self.complete_library_selection(
                    frame,
                    looked,
                    hand,
                    remainder,
                    source_block,
                    to_command,
                )?;
            }
            CommanderContinuation::BoardReturn {
                frame,
                objects,
                mut remaining,
                mut choices,
                source_block,
            } => {
                if remaining.first() != Some(&object) {
                    return Err("mass return choice order".into());
                }
                remaining.remove(0);
                if present {
                    choices.push((object, to_command));
                }
                self.continue_board_return(frame, objects, remaining, choices, source_block)?;
            }
            CommanderContinuation::SpellMove {
                mut frame,
                source_block,
            } => {
                if self.is_successor_tuck(&frame)? {
                    return self.complete_successor_tuck(frame, to_command);
                }
                let outcome = if !present {
                    InstructionOutcome::ObjectNoLongerPresent { target: object }
                } else {
                    InstructionOutcome::Moved {
                        movement: self.move_instruction_object(
                            object,
                            actual_destination,
                            BatchCause::Instruction {
                                stack_id: frame.spell.id,
                                player: frame.spell.controller,
                                source: frame.spell.source,
                                cursor: frame.cursor,
                                source_block,
                            },
                        )?,
                    }
                };
                frame.outcomes.push(outcome);
                frame.cursor = frame
                    .cursor
                    .checked_add(1)
                    .ok_or("instruction cursor resource limit")?;
                self.state.rules.frame = Frame::Resolve { frame };
            }
            CommanderContinuation::ResolutionDraw {
                frame,
                player,
                source_block,
            } => {
                let movement = if present {
                    Some(self.move_object_for(
                        object,
                        actual_destination,
                        BatchCause::Instruction {
                            stack_id: frame.spell.id,
                            player,
                            source: frame.spell.source,
                            cursor: frame.cursor,
                            source_block,
                        },
                    )?)
                } else {
                    None
                };
                self.finish_draw_attempt(
                    frame,
                    if present {
                        InstructionOutcome::Drawn {
                            player,
                            movement,
                            failed: false,
                        }
                    } else {
                        InstructionOutcome::PlayerActionSkipped { player }
                    },
                )?;
            }
            CommanderContinuation::Draw { player } => {
                if present {
                    self.move_object_for(
                        object,
                        actual_destination,
                        BatchCause::DrawStep { player },
                    )?;
                }
                self.grant_turn_priority()?;
            }
            CommanderContinuation::StateBasedBatch {
                mut remaining,
                mut choices,
                mut resume,
            } => {
                // CR101.4/101.4b and704.3: APNAP choices are public in sequence,
                // but no commander moves until every applicable owner chose.
                if present {
                    choices.push((object, to_command));
                }
                remaining.retain(|id| {
                    self.current(*id)
                        .is_ok_and(|o| matches!(o.zone, Zone::Graveyard { .. } | Zone::Exile))
                        && self.is_live(self.state.rules.cards[&id.card].owner)
                });
                if !remaining.is_empty() {
                    let object = remaining.remove(0);
                    self.state.rules.frame = Frame::CommanderReturn {
                        object,
                        destination: self.current(object)?.zone,
                        continuation: Box::new(CommanderContinuation::StateBasedBatch {
                            remaining,
                            choices,
                            resume,
                        }),
                    };
                } else {
                    let occurrences = choices
                        .iter()
                        .filter(|(id, yes)| {
                            *yes && self.current(*id).is_ok()
                                && self.is_live(self.state.rules.cards[&id.card].owner)
                        })
                        .map(|(id, _)| ProposedEvent::Move {
                            object: *id,
                            destination: Zone::Command {
                                owner: self.state.rules.cards[&id.card].owner,
                            },
                        })
                        .collect::<Vec<_>>();
                    if !occurrences.is_empty() {
                        self.commit_batch(
                            BatchCause::CommanderReturn,
                            EventGroup::Simultaneous { occurrences },
                        )?;
                        if let Frame::Stabilize { changed, .. } = &mut resume {
                            *changed = true;
                        }
                    }
                    self.state.rules.frame = resume;
                }
            }
            CommanderContinuation::StateBased { mut resume } => {
                if present && to_command {
                    self.move_object_for(
                        object,
                        Zone::Command { owner },
                        BatchCause::CommanderReturn,
                    )?;
                    if let Frame::Stabilize { changed, .. } = &mut resume {
                        *changed = true;
                    }
                }
                self.state.rules.frame = resume;
            }
        }
        Ok(())
    }
    pub(super) fn revalidate_commander_departure(&mut self) -> Result<(), String> {
        self.state.rules.commander_returns.retain(|id| {
            self.state
                .rules
                .objects
                .get(&id.card)
                .is_some_and(|o| o.id == *id)
        });
        if let Frame::CommanderReturn {
            object,
            destination,
            continuation,
        } = self.state.rules.frame.clone()
            && (self.current(object).is_err()
                || !self.is_live(self.state.rules.cards[&object.card].owner))
        {
            self.state.rules.pending = None;
            self.resume_commander_return(object, destination, *continuation, false)?;
        }
        Ok(())
    }
    pub(super) fn validate_commander_state(&self) -> Result<(), String> {
        for id in &self.state.rules.commander_returns {
            if self.state.rules.commanders.history(id.card).is_none()
                || !matches!(
                    self.current(*id)?.zone,
                    Zone::Graveyard { .. } | Zone::Exile
                )
            {
                return Err("snapshot commander return eligibility invariant".into());
            }
        }
        if let Frame::CommanderReturn {
            object,
            destination,
            continuation,
        } = &self.state.rules.frame
        {
            let owner = self
                .state
                .rules
                .cards
                .get(&object.card)
                .ok_or("snapshot commander physical identity invariant")?
                .owner;
            if self.state.rules.commanders.history(object.card).is_none() || !self.is_live(owner) {
                return Err("snapshot commander choice designation invariant".into());
            }
            let current = self.current(*object)?;
            match continuation.as_ref() {
                CommanderContinuation::CascadeBottom { progress } => {
                    self.validate_cascade_commander_choice(progress, *object, *destination)?;
                    if *destination != (Zone::Library { owner })
                        || current.zone != Zone::Exile
                        || progress.stage != cascade::CascadeStage::Bottom
                    {
                        return Err("snapshot cascade commander replacement invariant".into());
                    }
                }
                CommanderContinuation::GraveyardShuffle {
                    frame,
                    objects,
                    remaining,
                    choices,
                    source_block,
                } => {
                    self.validate_graveyard_shuffle_continuation(
                        frame,
                        objects,
                        remaining,
                        choices,
                        *source_block,
                    )?;
                    if remaining.first() != Some(object)
                        || *destination != (Zone::Library { owner })
                        || current.zone != (Zone::Graveyard { owner })
                    {
                        return Err("graveyard shuffle commander destination".into());
                    }
                }
                CommanderContinuation::HandReturn {
                    frame,
                    objects,
                    remaining,
                    choices,
                    source_block,
                } => {
                    self.validate_hand_return_continuation(
                        frame,
                        objects,
                        remaining,
                        choices,
                        *source_block,
                    )?;
                    if remaining.first() != Some(object)
                        || *destination != (Zone::Library { owner })
                        || current.zone != (Zone::Hand { owner })
                    {
                        return Err("hand return commander destination".into());
                    }
                }
                CommanderContinuation::LookTopMana { frame, revealed } => {
                    self.validate_resolving(frame)?;
                    self.validate_look_top_mana_move(frame, *object, *revealed)?;
                    if owner != frame.spell.controller
                        || *destination != (Zone::Hand { owner })
                        || current.zone != (Zone::Library { owner })
                    {
                        return Err("snapshot top card commander invariant".into());
                    }
                }
                CommanderContinuation::LibrarySelect {
                    frame,
                    looked,
                    hand,
                    remainder,
                    source_block,
                } => {
                    self.validate_resolving(frame)?;
                    self.validate_selection_choice(frame, owner, looked, *source_block)?;
                    self.validate_selection_partition(owner, looked, hand, remainder)?;
                    if hand.as_slice() != [*object]
                        || *destination != (Zone::Hand { owner })
                        || current.zone != (Zone::Library { owner })
                    {
                        return Err("snapshot selected commander invariant".into());
                    }
                }
                CommanderContinuation::BoardReturn {
                    frame,
                    objects,
                    remaining,
                    choices,
                    source_block,
                } => {
                    self.validate_board_return(frame, objects, remaining, choices, *source_block)?;
                    if remaining.first() != Some(object) || *destination != (Zone::Hand { owner }) {
                        return Err("mass return current choice".into());
                    }
                }
                CommanderContinuation::SpellMove {
                    frame,
                    source_block,
                } => {
                    if self.is_successor_tuck(frame)? {
                        let (expected, _, expected_block) =
                            self.successor_tuck_instruction(frame)?;
                        if *object != expected
                            || *source_block != expected_block
                            || *destination != (Zone::Library { owner })
                            || current.zone != (Zone::Graveyard { owner })
                            || frame.spell.controller != owner
                        {
                            return Err("snapshot commander successor tuck invariant".into());
                        }
                        return self.validate_resolving(frame);
                    }
                    let program = self.program_for(&frame.spell)?;
                    let instruction = program.instructions.get(frame.cursor as usize);
                    let (slot, expected_destination, expected_block) = match instruction {
                        Some(
                            Instruction::ReturnOwnGraveyardCreatureToHand {
                                target_slot,
                                source_block,
                                ..
                            }
                            | Instruction::ReturnOwnGraveyardCardToHand {
                                target_slot,
                                source_block,
                                ..
                            }
                            | Instruction::ReturnEntryTargetCreature {
                                target_slot,
                                source_block,
                                ..
                            }
                            | Instruction::ReturnToOwnerHand {
                                target_slot,
                                source_block,
                            },
                        ) => (*target_slot, Zone::Hand { owner }, *source_block),
                        Some(Instruction::PutOnTopOwnerLibrary {
                            target_slot,
                            source_block,
                        }) => (*target_slot, Zone::Library { owner }, *source_block),
                        _ => {
                            return Err(
                                "snapshot commander instruction continuation invariant".into()
                            );
                        }
                    };
                    let valid_target = if matches!(
                        instruction,
                        Some(Instruction::ReturnEntryTargetCreature { .. })
                    ) {
                        current.zone == Zone::Battlefield
                    } else {
                        self.object_target_legal(&frame.spell, *object)
                    };
                    if !valid_target
                        || *destination != expected_destination
                        || *source_block != expected_block
                        || frame.stack_id != frame.spell.id
                        || frame.spell.id.0 >= self.state.rules.next_stack_id
                        || frame.spell.paid.as_ref() != Some(&frame.spell.total_cost)
                        || frame.spell.targets.len() != program.targets.len()
                        || frame.legal_targets.len() != program.targets.len()
                        || frame.outcomes.len() != frame.cursor as usize
                        || frame.spell.targets.get(slot as usize) != Some(object)
                        || frame.legal_targets.get(slot as usize) != Some(&true)
                    {
                        return Err("snapshot commander spell continuation invariant".into());
                    }
                    self.validate_resolving(frame)?;
                }
                CommanderContinuation::ResolutionDraw {
                    frame,
                    player,
                    source_block,
                } => {
                    self.validate_resolving(frame)?;
                    if *player != owner
                        || !self
                            .resolution_draw_player(frame)
                            .is_ok_and(|p| p == *player)
                        || current.zone != (Zone::Library { owner })
                        || *destination != (Zone::Hand { owner })
                        || self
                            .state
                            .rules
                            .libraries
                            .get(player)
                            .and_then(|cards| cards.first())
                            != Some(&object.card)
                        || !match self
                            .program_for(&frame.spell)?
                            .instructions
                            .get(frame.cursor as usize)
                        {
                            Some(
                                Instruction::DrawControllerCards {
                                    source_block: b, ..
                                }
                                | Instruction::DrawEachPlayer {
                                    source_block: b, ..
                                },
                            ) => b == source_block && frame.draw_progress.is_some(),
                            Some(Instruction::DrawCards {
                                count,
                                source_block: b,
                                ..
                            }) => {
                                b == source_block
                                    && (*count == Quantity::from(1)
                                        || frame.draw_progress.is_some())
                            }
                            Some(
                                Instruction::OptionalDrawForSoleGreatestEntrant { source_block: b }
                                | Instruction::OptionalDrawCards { source_block: b }
                                | Instruction::OptionalDrawUnlessCasterPays {
                                    source_block: b, ..
                                },
                            ) => b == source_block,
                            Some(
                                Instruction::DrawDamageAmount { source_block: b }
                                | Instruction::DrawCardsIfAdditionalCost {
                                    source_block: b, ..
                                }
                                | Instruction::DrawAnnouncedX {
                                    source_block: b, ..
                                },
                            ) => b == source_block && frame.draw_progress.is_some(),
                            _ => false,
                        }
                    {
                        return Err(
                            "snapshot commander resolution draw continuation invariant".into()
                        );
                    }
                }
                CommanderContinuation::Draw { player } => {
                    if *player != owner
                        || self.state.rules.active != Some(*player)
                        || self.state.rules.turn.step != TurnStep::Draw
                        || current.zone != (Zone::Library { owner })
                        || *destination != (Zone::Hand { owner })
                        || self
                            .state
                            .rules
                            .libraries
                            .get(player)
                            .and_then(|cards| cards.first())
                            != Some(&object.card)
                    {
                        return Err("snapshot commander draw continuation invariant".into());
                    }
                }
                CommanderContinuation::StateBasedBatch {
                    remaining,
                    choices,
                    resume,
                } => {
                    let ids = choices
                        .iter()
                        .map(|(id, _)| *id)
                        .chain(std::iter::once(*object))
                        .chain(remaining.iter().copied())
                        .collect::<Vec<_>>();
                    if ids.len() > self.state.rules.commanders.records.len()
                        || ids.iter().collect::<BTreeSet<_>>().len() != ids.len()
                        || !matches!(resume, Frame::Stabilize { .. })
                        || current.zone != *destination
                        || !matches!(destination, Zone::Graveyard { .. } | Zone::Exile)
                    {
                        return Err("snapshot commander SBA group invariant".into());
                    }
                    let mut keys = vec![];
                    for id in ids {
                        let owner = self
                            .state
                            .rules
                            .cards
                            .get(&id.card)
                            .ok_or("snapshot commander group card")?
                            .owner;
                        if self.state.rules.commanders.history(id.card).is_none() {
                            return Err("snapshot commander group designation".into());
                        }
                        if self.is_live(owner)
                            && !self.current(id).is_ok_and(|o| {
                                matches!(o.zone, Zone::Graveyard { .. } | Zone::Exile)
                            })
                        {
                            return Err("snapshot commander group incarnation".into());
                        }
                        keys.push(((owner.0 + 4 - self.state.rules.turn_owner.0) % 4, id));
                    }
                    if !keys.windows(2).all(|w| w[0] < w[1]) {
                        return Err("snapshot commander group APNAP order".into());
                    }
                }
                CommanderContinuation::StateBased { resume } => {
                    if current.zone != *destination
                        || !matches!(destination, Zone::Graveyard { .. } | Zone::Exile)
                        || !matches!(resume, Frame::Stabilize { .. })
                    {
                        return Err("snapshot commander SBA continuation invariant".into());
                    }
                }
            }
        }
        Ok(())
    }
}
