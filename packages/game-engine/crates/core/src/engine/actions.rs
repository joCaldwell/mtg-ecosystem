use super::*;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "entry", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum StackKind {
    #[default]
    Spell,
    Triggered {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        match_condition: Option<InterveningCheck>,
        ability: u32,
        source_handles: Vec<ObjectHandle>,
        trigger_id: TriggerId,
        event: OccurrenceId,
        subject: ObjectId,
    },
    Activated {
        ability: u32,
        source_handles: Vec<ObjectHandle>,
    },
}
impl StackKind {
    pub(super) fn is_spell(&self) -> bool {
        matches!(self, Self::Spell)
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "choice", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum ResolutionChoice {
    Search {
        player: PlayerId,
        maximum: u32,
        source_block: u32,
    },
    Shuffle {
        player: PlayerId,
        index: u32,
        source_block: u32,
    },
    OptionalRecursion {
        player: PlayerId,
        target: ObjectId,
        source_block: u32,
    },
    Sacrifice {
        player: PlayerId,
        source_block: u32,
    },
    Discard {
        player: PlayerId,
        count: usize,
        source_block: u32,
    },
}
impl Game {
    pub(super) fn opponents(&self, player: PlayerId) -> Vec<PlayerId> {
        self.live_players()
            .into_iter()
            .filter(|p| *p != player)
            .collect()
    }
    pub(super) fn bound_target_count(&self, entry: &StackInstance) -> usize {
        entry.targets.len()
            + entry.player_targets.len()
            + entry.stack_targets.len()
            + entry.entity_targets.len()
    }
    pub(super) fn has_target_domain(&self, program: &SpellProgram, player: PlayerId) -> bool {
        if !program.modes.is_empty() {
            return program
                .modes
                .iter()
                .any(|mode| self.has_target_domain(&mode.program, player));
        }
        match program.targets.first().map(|target| target.domain) {
            None => true,
            Some(TargetDomain::AnyDamageTarget | TargetDomain::AnyPlayer) => {
                !self.live_players().is_empty()
            }
            Some(TargetDomain::OpponentPlayer) => !self.opponents(player).is_empty(),
            Some(
                domain
                @ (TargetDomain::StackSpell | TargetDomain::StackActivatedOrTriggeredAbility),
            ) => !self.eligible_stack_targets(domain, None).is_empty(),
            Some(TargetDomain::OwnGraveyardCreatureCard) => {
                !self.graveyard_creatures(player).is_empty()
            }
            Some(TargetDomain::BattlefieldCreature) => {
                self.state.objects.values().any(|o| self.target_legal(o.id))
            }
        }
    }
    pub(super) fn announced_target_legality(&self, entry: &StackInstance) -> Vec<bool> {
        if !entry.entity_targets.is_empty() {
            entry
                .entity_targets
                .iter()
                .map(|t| self.entity_legal(*t))
                .collect()
        } else if !entry.stack_targets.is_empty() {
            entry
                .stack_targets
                .iter()
                .enumerate()
                .map(|(slot, target)| {
                    self.program_for(entry).is_ok_and(|program| {
                        program.targets.get(slot).is_some_and(|spec| {
                            self.stack_target_legal(*target, entry.id, spec.domain)
                        })
                    })
                })
                .collect()
        } else if !entry.player_targets.is_empty() {
            entry
                .player_targets
                .iter()
                .enumerate()
                .map(|(slot, p)| self.player_target_legal(entry, slot, *p))
                .collect()
        } else {
            entry
                .targets
                .iter()
                .map(|target| self.object_target_legal(entry, *target))
                .collect()
        }
    }
    fn controlled_creatures(&self, player: PlayerId) -> Vec<ObjectId> {
        self.state
            .objects
            .values()
            .filter(|o| o.controller == player && self.target_legal(o.id))
            .map(|o| o.id)
            .collect()
    }
    fn action_hand(&self, player: PlayerId) -> Vec<ObjectId> {
        self.state
            .objects
            .values()
            .filter(|o| o.zone == (Zone::Hand { owner: player }))
            .map(|o| o.id)
            .collect()
    }
    fn validate_activation(
        &self,
        player: PlayerId,
        object: ObjectId,
        ability: u32,
    ) -> Result<(), String> {
        if !self.prepared.config.actions_profile() || self.state.stack.len() >= self.stack_limit() {
            return Err("unsupported activated ability profile/stack bound".into());
        }
        let source = self.current(object)?;
        let ability = self
            .definition(object.card)?
            .activated
            .get(ability as usize)
            .ok_or("unknown activated ability")?;
        if source.zone != Zone::Battlefield
            || source.controller != player
            || !self.is_live(player)
            || ability.functioning_zone != FunctioningZone::Battlefield
            || (!self.prepared.config.stack_costs_profile()
                && ability.cost != ActivationCost::SacrificeSource)
        {
            return Err("invalid activated ability source/permission".into());
        }
        let tap = ability.cost == ActivationCost::TapSource
            || matches!(&ability.cost, ActivationCost::Composite { costs } if costs.tap);
        if tap
            && (source.tapped
                || (self.characteristics(object)?.kind == CardKind::Creature
                    && !source.attack_ready
                    && !self.has_keyword(source.id, KeywordAbility::Haste)))
        {
            return Err("tap ability requires an untapped continuously controlled creature".into());
        }
        if !self.has_target_domain(&ability.program, player) {
            return Err("no eligible ability targets".into());
        }
        Ok(())
    }
    // CR 602.2, 113.7a/113.8: a nonmana ability is its own stack object;
    // sacrificing its source pays the cost without moving a card to the stack.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    pub(super) fn activate_ability(
        &mut self,
        player: PlayerId,
        decision_id: u64,
        object: ObjectId,
        ability: u32,
    ) -> Result<(), String> {
        self.validate_activation(player, object, ability)?;
        self.announce_ability(player, decision_id, object, ability)
    }
    pub(super) fn actions_request(&self) -> Result<Option<Request>, String> {
        let Frame::ResolutionChoice { choice, .. } = &self.state.frame else {
            return Ok(None);
        };
        let decision_id = self.state.revision;
        Ok(match choice {
            ResolutionChoice::Search {
                player, maximum, ..
            } if self.is_live(*player) => Some(Request::SearchLands {
                decision_id,
                player: *player,
                maximum: *maximum,
                candidates: self.search_lands(*player),
            }),
            ResolutionChoice::OptionalRecursion { player, .. } if self.is_live(*player) => {
                Some(Request::OptionalRecursion {
                    decision_id,
                    player: *player,
                })
            }
            ResolutionChoice::Sacrifice { player, .. } if self.is_live(*player) => {
                let candidates = self.controlled_creatures(*player);
                (!candidates.is_empty()).then_some(Request::Sacrifice {
                    decision_id,
                    player: *player,
                    candidates,
                })
            }
            ResolutionChoice::Discard { player, count, .. } if self.is_live(*player) => {
                let candidates = self.action_hand(*player);
                (!candidates.is_empty()).then_some(Request::ResolveDiscard {
                    decision_id,
                    player: *player,
                    count: std::cmp::min(*count, candidates.len()),
                    candidates,
                })
            }
            _ => None,
        })
    }
    pub(super) fn validate_actions_response(
        &self,
        player: PlayerId,
        response: &Response,
    ) -> Option<Result<(), String>> {
        if let Some(result) = self.validate_staples_response(player, response) {
            return Some(result);
        }
        if let Some(result) = self.validate_trigger_response(player, response) {
            return Some(result);
        }
        match (&self.state.frame, response) {
            (Frame::EntityTargets { .. }, Response::EntityTargets { entities }) => {
                Some(self.validate_entity_targets(player, entities))
            }
            (Frame::StackTargets { .. }, Response::StackTargets { entries }) => {
                Some(self.validate_stack_targets(player, entries))
            }
            (Frame::Priority, Response::ActivateAbility { object, ability })
                if self.state.priority == player =>
            {
                Some(self.validate_activation(player, *object, *ability))
            }
            (
                Frame::PlayerTargets { .. } | Frame::TriggerTargets { .. },
                Response::PlayerTargets { players },
            ) => Some((|| {
                let top = self.top()?;
                let caster = top.controller;
                if player != caster
                    || players.len() != self.program()?.targets.len()
                    || players
                        .iter()
                        .enumerate()
                        .any(|(slot, p)| !self.player_target_legal(top, slot, *p))
                {
                    return Err("invalid player targets".into());
                }
                Ok(())
            })()),
            (
                Frame::ResolutionChoice {
                    choice:
                        ResolutionChoice::Sacrifice {
                            player: chooser, ..
                        },
                    ..
                },
                Response::Sacrifice { object },
            ) => Some(
                if *chooser == player
                    && self.is_live(player)
                    && self.controlled_creatures(player).contains(object)
                {
                    Ok(())
                } else {
                    Err("invalid sacrifice choice".into())
                },
            ),
            (
                Frame::ResolutionChoice {
                    choice:
                        ResolutionChoice::Discard {
                            player: chooser,
                            count,
                            ..
                        },
                    ..
                },
                Response::ResolveDiscard { objects },
            ) => Some((|| {
                let hand = self.action_hand(player);
                if *chooser != player
                    || !self.is_live(player)
                    || objects.len() != std::cmp::min(*count, hand.len())
                    || objects.iter().collect::<BTreeSet<_>>().len() != objects.len()
                    || objects.iter().any(|o| !hand.contains(o))
                {
                    return Err("invalid resolution discard choice".into());
                }
                Ok(())
            })()),
            _ => None,
        }
    }
    pub(super) fn actions_step(&mut self) -> Result<(), String> {
        match self.state.frame.clone() {
            Frame::EntityTargets { cast } => self.bind_entity_targets(cast)?,
            Frame::StackTargets { cast } => self.bind_stack_targets(cast)?,
            Frame::PlayerTargets { cast } => {
                let input = self.state.pending.take().ok_or("missing player targets")?;
                let Response::PlayerTargets { players } = input.response else {
                    return Err("player target response invariant".into());
                };
                self.state
                    .stack
                    .last_mut()
                    .ok_or("missing player-target stack entry")?
                    .player_targets = players;
                self.state.frame = Frame::Mana { cast };
            }
            Frame::ResolutionChoice { frame, choice } => {
                if matches!(
                    choice,
                    ResolutionChoice::Search { .. }
                        | ResolutionChoice::Shuffle { .. }
                        | ResolutionChoice::OptionalRecursion { .. }
                ) {
                    return self.staples_choice_step(frame, choice);
                }
                let Some(request) = self.actions_request()? else {
                    let outcome = match choice {
                        ResolutionChoice::Search { .. }
                        | ResolutionChoice::Shuffle { .. }
                        | ResolutionChoice::OptionalRecursion { .. } => unreachable!(),
                        ResolutionChoice::Sacrifice { player, .. } => {
                            InstructionOutcome::Sacrificed {
                                player,
                                movement: None,
                            }
                        }
                        ResolutionChoice::Discard { player, .. } => InstructionOutcome::Discarded {
                            player,
                            movements: vec![],
                        },
                    };
                    self.state.pending = None;
                    self.finish_instruction(frame, outcome)?;
                    return Ok(());
                };
                let input = self
                    .state
                    .pending
                    .take()
                    .ok_or("missing resolution choice")?;
                self.validate_choice_input(&choice, input.player, &input.response)?;
                let source_block = match choice {
                    ResolutionChoice::Search { .. }
                    | ResolutionChoice::Shuffle { .. }
                    | ResolutionChoice::OptionalRecursion { .. } => unreachable!(),
                    ResolutionChoice::Sacrifice { source_block, .. }
                    | ResolutionChoice::Discard { source_block, .. } => source_block,
                };
                let cause = BatchCause::Instruction {
                    stack_id: frame.spell.id,
                    player: frame.spell.controller,
                    source: frame.spell.source,
                    cursor: frame.cursor,
                    source_block,
                };
                let outcome = match input.response {
                    Response::Sacrifice { object } => {
                        let EventOutcome::Sacrificed { movement, .. } = self.commit(
                            ProposedEvent::Sacrifice {
                                object,
                                player: request.player(),
                            },
                            cause,
                        )?
                        else {
                            return Err("missing sacrifice outcome".into());
                        };
                        InstructionOutcome::Sacrificed {
                            player: request.player(),
                            movement: Some(movement),
                        }
                    }
                    Response::ResolveDiscard { objects } => {
                        let mut movements = vec![];
                        for object in objects {
                            let EventOutcome::CardDiscarded { movement, .. } = self.commit(
                                ProposedEvent::DiscardCard {
                                    object,
                                    player: request.player(),
                                },
                                cause.clone(),
                            )?
                            else {
                                return Err("missing discard outcome".into());
                            };
                            movements.push(movement);
                        }
                        InstructionOutcome::Discarded {
                            player: request.player(),
                            movements,
                        }
                    }
                    _ => return Err("resolution response invariant".into()),
                };
                self.finish_instruction(frame, outcome)?;
            }
            _ => return Err("action frame invariant".into()),
        }
        Ok(())
    }
    fn validate_choice_input(
        &self,
        choice: &ResolutionChoice,
        player: PlayerId,
        response: &Response,
    ) -> Result<(), String> {
        let Frame::ResolutionChoice {
            choice: current, ..
        } = &self.state.frame
        else {
            return Err("missing resolution choice frame".into());
        };
        if choice != current {
            return Err("resolution choice changed".into());
        }
        self.validate_actions_response(player, response)
            .ok_or("resolution choice response mismatch")?
    }
    pub(super) fn finish_instruction(
        &mut self,
        mut frame: Resolving,
        outcome: InstructionOutcome,
    ) -> Result<(), String> {
        frame.outcomes.push(outcome);
        frame.cursor = frame
            .cursor
            .checked_add(1)
            .ok_or("instruction cursor resource limit")?;
        self.state.frame = Frame::Resolve { frame };
        Ok(())
    }
    pub(super) fn execute_action_instruction(
        &mut self,
        frame: Resolving,
        instruction: Instruction,
    ) -> Result<(), String> {
        if !self.prepared.config.actions_profile() {
            return Err("unsupported resolution instruction profile".into());
        }
        let player = frame.spell.controller;
        match instruction {
            op @ (Instruction::DestroyTargetCreature { .. }
            | Instruction::DestroyAllCreatures { .. }
            | Instruction::ReturnOwnGraveyardCreatureToHand { .. }
            | Instruction::SearchBasicLands { .. }) => self.execute_staples_instruction(frame, op),
            op @ (Instruction::CreateCreatureTokens { .. }
            | Instruction::PutPlusOneCounters { .. }) => self.execute_token_instruction(frame, op),
            op @ (Instruction::ModifyPowerToughness { .. }
            | Instruction::PreventCombatDamage { .. }
            | Instruction::PreventNextDamage { .. }) => self.execute_effect_instruction(frame, op),
            op @ (Instruction::DealDamage { .. }
            | Instruction::GainLife { .. }
            | Instruction::LoseLife { .. }) => self.execute_damage_life_instruction(frame, op),
            Instruction::TargetPlayerSacrificeCreature {
                target_slot,
                source_block,
            } => {
                let target = *frame
                    .spell
                    .player_targets
                    .get(target_slot as usize)
                    .ok_or("missing player target binding")?;
                if !frame
                    .legal_targets
                    .get(target_slot as usize)
                    .copied()
                    .unwrap_or(false)
                    || !self.is_live(target)
                    || self.controlled_creatures(target).is_empty()
                {
                    self.finish_instruction(
                        frame,
                        InstructionOutcome::Sacrificed {
                            player: target,
                            movement: None,
                        },
                    )
                } else {
                    self.state.frame = Frame::ResolutionChoice {
                        frame,
                        choice: ResolutionChoice::Sacrifice {
                            player: target,
                            source_block,
                        },
                    };
                    Ok(())
                }
            }
            Instruction::DrawCards {
                recipient,
                count,
                source_block,
            } => {
                if count != Quantity::from(1) {
                    return Err("unsupported draw count".into());
                }
                let player = match recipient {
                    None | Some(LifeRecipient::Controller) => player,
                    Some(LifeRecipient::Target { target_slot }) => {
                        if frame.legal_targets.get(target_slot as usize) != Some(&true) {
                            return self.finish_instruction(
                                frame,
                                InstructionOutcome::Effect { actual: None },
                            );
                        }
                        *frame
                            .spell
                            .player_targets
                            .get(target_slot as usize)
                            .ok_or("missing draw player target")?
                    }
                };
                if !self.is_live(player) {
                    return self.finish_instruction(
                        frame,
                        InstructionOutcome::PlayerActionSkipped { player },
                    );
                }
                if let Some(card) = self
                    .state
                    .libraries
                    .get(&player)
                    .and_then(|cards| cards.first())
                    .copied()
                {
                    let object = self
                        .object_id(card)
                        .ok_or("missing instruction draw source")?;
                    if self.state.commanders.history(card).is_some() {
                        self.state.frame = Frame::CommanderReturn {
                            object,
                            destination: Zone::Hand { owner: player },
                            continuation: Box::new(CommanderContinuation::ResolutionDraw {
                                frame,
                                player,
                                source_block,
                            }),
                        };
                        Ok(())
                    } else {
                        let movement = self.move_object_for(
                            object,
                            Zone::Hand { owner: player },
                            BatchCause::Instruction {
                                stack_id: frame.spell.id,
                                player,
                                source: frame.spell.source,
                                cursor: frame.cursor,
                                source_block,
                            },
                        )?;
                        self.finish_instruction(
                            frame,
                            InstructionOutcome::Drawn {
                                player,
                                movement: Some(movement),
                                failed: false,
                            },
                        )
                    }
                } else {
                    self.commit(
                        ProposedEvent::FailedDraw { player },
                        BatchCause::Instruction {
                            stack_id: frame.spell.id,
                            player,
                            source: frame.spell.source,
                            cursor: frame.cursor,
                            source_block,
                        },
                    )?;
                    self.finish_instruction(
                        frame,
                        InstructionOutcome::Drawn {
                            player,
                            movement: None,
                            failed: true,
                        },
                    )
                }
            }
            Instruction::DiscardCards {
                count,
                source_block,
            } => {
                if count != Quantity::from(1) {
                    return Err("unsupported discard count".into());
                }
                if !self.is_live(player) {
                    return self.finish_instruction(
                        frame,
                        InstructionOutcome::PlayerActionSkipped { player },
                    );
                }
                if self.action_hand(player).is_empty() {
                    self.finish_instruction(
                        frame,
                        InstructionOutcome::Discarded {
                            player,
                            movements: vec![],
                        },
                    )
                } else {
                    self.state.frame = Frame::ResolutionChoice {
                        frame,
                        choice: ResolutionChoice::Discard {
                            player,
                            count: 1,
                            source_block,
                        },
                    };
                    Ok(())
                }
            }
            Instruction::CounterStackObject {
                target_slot,
                source_block,
            } => self.resolve_counter_instruction(frame, target_slot, source_block),
            _ => Err("unexpected action instruction".into()),
        }
    }
    pub(super) fn finish_ability(
        &mut self,
        entry: &StackInstance,
        resolved: bool,
    ) -> Result<(), String> {
        if self
            .state
            .stack
            .last()
            .is_some_and(|top| top.id == entry.id)
        {
            self.state.stack.pop();
        } else if self.is_live(entry.controller)
            || self.state.stack.iter().any(|item| item.id == entry.id)
        {
            return Err("ability completion binding invariant".into());
        }
        if matches!(entry.kind, StackKind::Triggered { .. }) {
            self.record_event(Event::TriggeredAbilityCompleted {
                stack_id: entry.id,
                resolved,
            })?;
        } else {
            self.record_event(Event::AbilityCompleted { stack_id: entry.id })?;
        }
        self.state.frame = Frame::Priority;
        self.state.priority = self
            .state
            .active
            .or_else(|| self.next_live(self.state.turn_owner))
            .ok_or("no live player after ability")?;
        self.stabilize()
    }
    pub(super) fn invocation_lives(&self, entry: &StackInstance) -> bool {
        if entry.kind.is_spell() {
            self.state
                .cards
                .get(&entry.source.card)
                .is_some_and(|card| self.is_live(card.owner))
        } else {
            self.is_live(entry.controller)
        }
    }
    pub(super) fn validate_resolving(&self, frame: &Resolving) -> Result<(), String> {
        self.validate_mode_binding(&frame.spell)?;
        self.validate_condition_cache(frame)?;
        let entry = &frame.spell;
        let program = self.program_for(entry)?;
        self.validate_damage_bindings(entry)?;
        if entry.id != frame.stack_id
            || entry.id.0 >= self.state.next_stack_id
            || self.bound_target_count(entry) != program.targets.len()
            || entry.paid.as_ref() != Some(&entry.total_cost)
            || frame.cursor as usize > program.instructions.len()
            || frame.outcomes.len() != frame.cursor as usize
            || frame.legal_targets.len() != program.targets.len()
        {
            return Err("snapshot resolving invocation invariant".into());
        }
        if self.state.stack.iter().any(|item| item.id == entry.id) {
            if self.top()? != entry {
                return Err("snapshot resolving top binding".into());
            }
        } else if self.invocation_lives(entry) {
            return Err("snapshot detached invocation invariant".into());
        }
        Ok(())
    }
    pub(super) fn revalidate_actions_departure(&mut self) -> Result<(), String> {
        if matches!(self.state.frame, Frame::ResolutionChoice { .. })
            && self.state.pending.as_ref().is_some_and(|input| {
                self.validate_response(input.player, &input.response)
                    .is_err()
            })
        {
            self.state.pending = None;
        }
        Ok(())
    }
    pub(super) fn validate_actions_state(&self) -> Result<(), String> {
        for entry in &self.state.stack {
            let StackKind::Activated {
                ability,
                source_handles,
            } = &entry.kind
            else {
                continue;
            };
            self.validate_activated_entry(entry, *ability, source_handles)?;
        }
        if let Frame::ResolutionChoice { frame, choice } = &self.state.frame {
            if !self.prepared.config.actions_profile() {
                return Err("snapshot resolution choice profile invariant".into());
            }
            let instruction = self
                .program_for(&frame.spell)?
                .instructions
                .get(frame.cursor as usize)
                .ok_or("snapshot missing choice instruction")?;
            let valid = match (choice, instruction) {
                (
                    ResolutionChoice::Sacrifice {
                        player,
                        source_block,
                    },
                    Instruction::TargetPlayerSacrificeCreature {
                        target_slot,
                        source_block: expected,
                    },
                ) => {
                    *source_block == *expected
                        && frame.spell.player_targets.get(*target_slot as usize) == Some(player)
                }
                (
                    ResolutionChoice::Discard {
                        player,
                        count,
                        source_block,
                    },
                    Instruction::DiscardCards {
                        count: expected,
                        source_block: expected_block,
                    },
                ) => {
                    *player == frame.spell.controller
                        && *count == 1
                        && *expected == Quantity::from(1)
                        && *source_block == *expected_block
                }
                _ => self.valid_staples_choice(frame, choice, instruction),
            };
            if !valid {
                return Err("snapshot resolution choice continuation invariant".into());
            }
        }
        Ok(())
    }
}
