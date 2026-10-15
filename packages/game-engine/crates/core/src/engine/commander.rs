use super::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "continuation", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum CommanderContinuation {
    Hand { frame: Resolving, source_block: u32 },
    StateBased { resume: Frame },
}

impl Game {
    pub(super) fn commander_enabled(&self) -> bool {
        self.prepared.config.commander_profile()
    }
    // CR 903.8 / 601.2f: the additional generic cost uses completed prior
    // casts of this physical commander by this caster, only from command.
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
    // CR 903.9a / 704.3: a fresh graveyard/exile incarnation is eligible at
    // its next SBA check only. Declining consumes eligibility permanently.
    // There is one physical commander in these exact admitted scenarios.
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
        match continuation {
            // CR 903.9b: replace the hand move before it happens; resolution
            // waits for the owner, then retains the actual movement outcome.
            CommanderContinuation::Hand {
                mut frame,
                source_block,
            } => {
                let outcome = if !present {
                    InstructionOutcome::ObjectNoLongerPresent { target: object }
                } else {
                    InstructionOutcome::Moved {
                        movement: self.move_object_for(
                            object,
                            if to_command {
                                Zone::Command { owner }
                            } else {
                                destination
                            },
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
                CommanderContinuation::Hand {
                    frame,
                    source_block,
                } => {
                    if current.zone != Zone::Battlefield
                        || *destination != (Zone::Hand { owner })
                        || frame.outcomes.len() != frame.cursor as usize
                        || self
                            .program_for(&frame.spell)?
                            .instructions
                            .get(frame.cursor as usize)
                            != Some(&Instruction::ReturnToOwnerHand {
                                target_slot: 0,
                                source_block: *source_block,
                            })
                        || frame.spell.targets.first() != Some(object)
                        || frame.legal_targets.first() != Some(&true)
                    {
                        return Err("snapshot commander hand continuation invariant".into());
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
