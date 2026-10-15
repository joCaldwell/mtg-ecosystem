use super::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "continuation", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum CommanderContinuation {
    SpellMove { frame: Resolving, source_block: u32 },
    Draw { player: PlayerId },
    StateBased { resume: Frame },
}

impl Game {
    pub(super) fn commander_enabled(&self) -> bool {
        self.prepared.config.commander_profile()
    }
    pub(super) fn commander_damage_enabled(&self) -> bool {
        self.prepared.config.damage_profile() || self.prepared.config.library_profile()
    }
    pub(super) fn commander_designation(
        &self,
        viewer: PlayerId,
        card: CardId,
    ) -> Result<String, String> {
        self.state
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
        let mut cost = self.definition(card)?.characteristics.mana_cost.clone();
        if matches!(from, Zone::Command { .. }) {
            if !self.commander_enabled() || from != (Zone::Command { owner: caster }) {
                return Err("unsupported command-zone casting permission".into());
            }
            cost.generic.add(&self.state.commanders.tax(card, caster)?);
        }
        Ok(cost)
    }
    // CR 903.9a / 704.3: consume a fresh graveyard/exile incarnation's next
    // SBA opportunity even if declined. Exact admission permits one commander.
    pub(super) fn offer_commander_sba(&mut self, resume: Frame) -> Result<bool, String> {
        let eligible = self.state.commander_returns.iter().copied().find(|id| {
            self.current(*id)
                .is_ok_and(|o| matches!(o.zone, Zone::Graveyard { .. } | Zone::Exile))
                && self.is_live(self.state.cards[&id.card].owner)
        });
        self.state.commander_returns.clear();
        if let Some(object) = eligible {
            self.state.frame = Frame::CommanderReturn {
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
        if !self.commander_enabled() || self.state.commanders.history(card).is_none() {
            return Ok(false);
        }
        let object = self
            .object_id(card)
            .ok_or("missing commander draw source")?;
        if self.current(object)?.zone != (Zone::Library { owner: player }) {
            return Err("commander draw source invariant".into());
        }
        self.state.frame = Frame::CommanderReturn {
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
            .pending
            .take()
            .ok_or("missing commander return choice")?;
        let Response::CommanderReturn { to_command } = input.response else {
            return Err("commander return response invariant".into());
        };
        if input.player != self.state.cards[&object.card].owner {
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
        let owner = self.state.cards[&object.card].owner;
        let present = self.current(object).is_ok() && self.is_live(owner);
        let actual_destination = if to_command {
            Zone::Command { owner }
        } else {
            destination
        };
        match continuation {
            CommanderContinuation::SpellMove {
                mut frame,
                source_block,
            } => {
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
                self.state.frame = Frame::Resolve { frame };
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
                self.state.frame = resume;
            }
        }
        Ok(())
    }
    pub(super) fn revalidate_commander_departure(&mut self) -> Result<(), String> {
        self.state.commander_returns.retain(|id| {
            self.state
                .objects
                .get(&id.card)
                .is_some_and(|o| o.id == *id)
        });
        if let Frame::CommanderReturn {
            object,
            destination,
            continuation,
        } = self.state.frame.clone()
            && (self.current(object).is_err()
                || !self.is_live(self.state.cards[&object.card].owner))
        {
            self.state.pending = None;
            self.resume_commander_return(object, destination, *continuation, false)?;
        }
        Ok(())
    }
    pub(super) fn validate_commander_state(&self) -> Result<(), String> {
        for id in &self.state.commander_returns {
            if !self.commander_enabled()
                || self.state.commanders.history(id.card).is_none()
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
        } = &self.state.frame
        {
            let owner = self
                .state
                .cards
                .get(&object.card)
                .ok_or("snapshot commander physical identity invariant")?
                .owner;
            if !self.commander_enabled()
                || self.state.commanders.history(object.card).is_none()
                || !self.is_live(owner)
            {
                return Err("snapshot commander choice designation invariant".into());
            }
            let current = self.current(*object)?;
            match continuation.as_ref() {
                CommanderContinuation::SpellMove {
                    frame,
                    source_block,
                } => {
                    let program = self.program_for(&frame.spell)?;
                    let (slot, expected_destination, expected_block) =
                        match program.instructions.get(frame.cursor as usize) {
                            Some(Instruction::ReturnToOwnerHand {
                                target_slot,
                                source_block,
                            }) => (*target_slot, Zone::Hand { owner }, *source_block),
                            Some(Instruction::PutOnTopOwnerLibrary {
                                target_slot,
                                source_block,
                            }) if self.prepared.config.library_profile() => {
                                (*target_slot, Zone::Library { owner }, *source_block)
                            }
                            _ => {
                                return Err(
                                    "snapshot commander instruction continuation invariant".into(),
                                );
                            }
                        };
                    if current.zone != Zone::Battlefield
                        || *destination != expected_destination
                        || *source_block != expected_block
                        || frame.stack_id != frame.spell.id
                        || frame.spell.id.0 >= self.state.next_stack_id
                        || frame.spell.paid.as_ref() != Some(&frame.spell.total_cost)
                        || frame.spell.targets.len() != program.targets.len()
                        || frame.legal_targets.len() != program.targets.len()
                        || frame.outcomes.len() != frame.cursor as usize
                        || frame.spell.targets.get(slot as usize) != Some(object)
                        || frame.legal_targets.get(slot as usize) != Some(&true)
                    {
                        return Err("snapshot commander spell continuation invariant".into());
                    }
                    if self.current(frame.spell.source).is_ok() {
                        if self.top()? != &frame.spell {
                            return Err("snapshot commander resolving top binding".into());
                        }
                    } else if self.is_live(
                        self.state
                            .cards
                            .get(&frame.spell.source.card)
                            .ok_or("snapshot missing resolving physical card")?
                            .owner,
                    ) || self
                        .state
                        .stack
                        .iter()
                        .any(|entry| entry.id == frame.spell.id)
                    {
                        return Err("snapshot commander detached invocation invariant".into());
                    }
                }
                CommanderContinuation::Draw { player } => {
                    if *player != owner
                        || self.state.active != Some(*player)
                        || self.state.turn.step != TurnStep::Draw
                        || current.zone != (Zone::Library { owner })
                        || *destination != (Zone::Hand { owner })
                        || self
                            .state
                            .libraries
                            .get(player)
                            .and_then(|cards| cards.first())
                            != Some(&object.card)
                    {
                        return Err("snapshot commander draw continuation invariant".into());
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
