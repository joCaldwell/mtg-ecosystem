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
        event: TriggerOccurrenceId,
        witness: TriggerEventWitness,
    },
    Activated {
        ability: u32,
        source_handles: Vec<ObjectHandle>,
    },
}
impl StackKind {
    pub(super) fn event_player(&self) -> Result<Option<PlayerId>, String> {
        match self {
            Self::Triggered {
                witness:
                    witness @ (TriggerEventWitness::StepBegan { .. }
                    | TriggerEventWitness::SpellCast { .. }),
                ..
            } => Ok(witness.event_player()),
            _ => Err("unbound event-player recipient".into()),
        }
    }
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
    OptionalEffect {
        player: PlayerId,
        target: ObjectId,
        source_block: u32,
    },
    OptionalDraw {
        player: PlayerId,
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
            Some(TargetDomain::StackSpell | TargetDomain::StackActivatedOrTriggeredAbility) => {
                !self
                    .eligible_stack_targets_for(&program.targets[0], None)
                    .is_empty()
            }
            Some(TargetDomain::BattlefieldPermanent) => self
                .state
                .rules
                .objects
                .values()
                .any(|o| self.permanent_target_legal(o.id, player, &program.targets[0])),
            Some(TargetDomain::OwnGraveyardCreatureCard) => {
                !self.graveyard_creatures(player).is_empty()
            }
            Some(TargetDomain::BlockingBattlefieldCreature) => self
                .state
                .rules
                .blocking_creatures
                .iter()
                .any(|o| self.is_blocking_creature(*o) && self.targetable_by(*o, player)),
            Some(
                domain @ (TargetDomain::BattlefieldCreature
                | TargetDomain::ControlledBattlefieldCreature),
            ) => self.state.rules.objects.values().any(|o| {
                self.is_battlefield_creature(o.id)
                    && self.targetable_by(o.id, player)
                    && (domain != TargetDomain::ControlledBattlefieldCreature
                        || o.controller == player)
            }),
        }
    }
    pub(super) fn announced_target_legality(&self, entry: &StackInstance) -> Vec<bool> {
        if !entry.entity_targets.is_empty() {
            entry
                .entity_targets
                .iter()
                .map(|t| self.entity_target_legal(*t, entry.controller))
                .collect()
        } else if !entry.stack_targets.is_empty() {
            entry
                .stack_targets
                .iter()
                .enumerate()
                .map(|(slot, target)| {
                    self.program_for(entry).is_ok_and(|program| {
                        program.targets.get(slot).is_some_and(|spec| {
                            self.eligible_stack_targets_for(spec, Some(entry.id))
                                .contains(target)
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
    pub(super) fn controlled_creatures(&self, player: PlayerId) -> Vec<ObjectId> {
        self.state
            .rules
            .objects
            .values()
            .filter(|o| o.controller == player && self.is_battlefield_creature(o.id))
            .map(|o| o.id)
            .collect()
    }
    // CR 603.3a/608.2c/701.9a-b/701.21a: the damaged player is frozen by the
    // actual trigger occurrence. The invocation's controller remains "you";
    // neither source control changes nor departures rebind either player.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    pub(super) fn resolution_player(
        &self,
        frame: &Resolving,
        recipient: Option<&ActionRecipient>,
    ) -> Result<PlayerId, String> {
        match recipient {
            None | Some(ActionRecipient::Controller) => Ok(frame.spell.controller),
            Some(ActionRecipient::EventPlayer) => match &frame.spell.kind {
                StackKind::Triggered {
                    witness: TriggerEventWitness::DamageDealt { damage, .. },
                    ..
                } => match damage.recipient {
                    DamageRecipient::Player { player } => Ok(player),
                    DamageRecipient::Creature { .. } => {
                        Err("action event requires a player damage recipient".into())
                    }
                },
                _ => Err("unbound damage-recipient action".into()),
            },
        }
    }
    fn action_hand(&self, player: PlayerId) -> Vec<ObjectId> {
        self.state
            .rules
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
        {
            return Err("invalid activated ability source/permission".into());
        }
        if ability.program.timing == SpellTiming::MainPhase && !self.main_phase_action(player) {
            return Err("equip requires sorcery timing".into());
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
        if let ActivationCost::SacrificePermanent { filter, .. } = &ability.cost
            && (self.sacrifice_candidates(player, filter).is_empty())
        {
            return Err("no payable sacrifice permanent".into());
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
        self.check_stack_capacity(1)?;
        self.validate_activation(player, object, ability)?;
        self.announce_ability(player, decision_id, object, ability)
    }
    pub(super) fn actions_request(&self) -> Result<Option<Request>, String> {
        let Frame::ResolutionChoice { choice, .. } = &self.state.rules.frame else {
            return Ok(None);
        };
        let decision_id = self.state.rules.revision;
        Ok(match choice {
            ResolutionChoice::Search {
                player, maximum, ..
            } if self.is_live(*player) => Some(Request::SearchLands {
                decision_id,
                player: *player,
                maximum: *maximum,
                candidates: self.search_lands(*player),
            }),
            ResolutionChoice::OptionalEffect { player, .. }
            | ResolutionChoice::OptionalDraw { player, .. }
                if self.is_live(*player) =>
            {
                Some(Request::OptionalEffect {
                    decision_id,
                    player: *player,
                })
            }
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
        if let (
            Frame::ResolutionChoice {
                choice:
                    ResolutionChoice::OptionalEffect {
                        player: chooser, ..
                    }
                    | ResolutionChoice::OptionalDraw {
                        player: chooser, ..
                    },
                ..
            },
            Response::OptionalEffect { .. },
        ) = (&self.state.rules.frame, response)
        {
            return Some(if *chooser == player && self.is_live(player) {
                Ok(())
            } else {
                Err("invalid optional effect choice".into())
            });
        }
        if let Some(result) = self.validate_staples_response(player, response) {
            return Some(result);
        }
        if let Some(result) = self.validate_trigger_response(player, response) {
            return Some(result);
        }
        match (&self.state.rules.frame, response) {
            (
                Frame::EntityTargets { .. } | Frame::TriggerTargets { .. },
                Response::EntityTargets { entities },
            ) => Some(self.validate_entity_targets(player, entities)),
            (Frame::StackTargets { .. }, Response::StackTargets { entries }) => {
                Some(self.validate_stack_targets(player, entries))
            }
            (Frame::Priority, Response::ActivateAbility { object, ability })
                if self.state.rules.priority == player =>
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
        match self.state.rules.frame.clone() {
            Frame::EntityTargets { cast } => self.bind_entity_targets(cast)?,
            Frame::StackTargets { cast } => self.bind_stack_targets(cast)?,
            Frame::PlayerTargets { cast } => {
                let input = self
                    .state
                    .rules
                    .pending
                    .take()
                    .ok_or("missing player targets")?;
                let Response::PlayerTargets { players } = input.response else {
                    return Err("player target response invariant".into());
                };
                self.state
                    .rules
                    .stack
                    .last_mut()
                    .ok_or("missing player-target stack entry")?
                    .player_targets = players;
                self.state.rules.frame = Frame::Mana { cast };
            }
            Frame::ResolutionChoice { frame, choice } => {
                if let ResolutionChoice::OptionalEffect {
                    player,
                    target,
                    source_block,
                } = choice
                {
                    return self.removal_choice_step(frame, player, target, source_block);
                }
                if matches!(
                    choice,
                    ResolutionChoice::OptionalEffect { .. }
                        | ResolutionChoice::Search { .. }
                        | ResolutionChoice::Shuffle { .. }
                        | ResolutionChoice::OptionalRecursion { .. }
                        | ResolutionChoice::OptionalDraw { .. }
                ) {
                    return self.staples_choice_step(frame, choice);
                }
                let Some(request) = self.actions_request()? else {
                    let outcome = match choice {
                        ResolutionChoice::OptionalEffect { .. }
                        | ResolutionChoice::Search { .. }
                        | ResolutionChoice::Shuffle { .. }
                        | ResolutionChoice::OptionalRecursion { .. }
                        | ResolutionChoice::OptionalDraw { .. } => unreachable!(),
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
                    self.state.rules.pending = None;
                    self.finish_instruction(frame, outcome)?;
                    return Ok(());
                };
                let input = self
                    .state
                    .rules
                    .pending
                    .take()
                    .ok_or("missing resolution choice")?;
                self.validate_choice_input(&choice, input.player, &input.response)?;
                let source_block = match choice {
                    ResolutionChoice::OptionalEffect { .. }
                    | ResolutionChoice::Search { .. }
                    | ResolutionChoice::Shuffle { .. }
                    | ResolutionChoice::OptionalRecursion { .. }
                    | ResolutionChoice::OptionalDraw { .. } => unreachable!(),
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
        } = &self.state.rules.frame
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
        if frame.damage_draw.is_some() {
            return Err("cannot advance an unfinished damage-quantity draw".into());
        }
        frame.outcomes.push(outcome);
        frame.cursor = frame
            .cursor
            .checked_add(1)
            .ok_or("instruction cursor resource limit")?;
        self.state.rules.frame = Frame::Resolve { frame };
        Ok(())
    }
    pub(super) fn execute_action_instruction(
        &mut self,
        frame: Resolving,
        instruction: Instruction,
    ) -> Result<(), String> {
        let player = frame.spell.controller;
        match instruction {
            i @ (Instruction::MoveSet { .. }
            | Instruction::PumpSet { .. }
            | Instruction::DamageSet { .. }
            | Instruction::GainLifeDestroyed { .. }
            | Instruction::ExileDamagedThisTurn { .. }
            | Instruction::EachPlayerSacrifice { .. }
            | Instruction::LoseLifeDeathController { .. }) => {
                self.execute_wipes_instruction(frame, i)
            }
            i @ (Instruction::SacrificeDamage { .. }
            | Instruction::EachOpponentSacrifice { .. }
            | Instruction::ModifySourcePowerToughness { .. }) => {
                self.execute_sacrifice_instruction(frame, i)
            }
            i @ (Instruction::DestroyPermanent { .. }
            | Instruction::ExilePermanent { .. }
            | Instruction::ChangeLifeFromTarget { .. }
            | Instruction::CreateTokensFor { .. }
            | Instruction::DamageTargetController { .. }) => {
                self.execute_removal_instruction(frame, i)
            }
            i @ (Instruction::AttachEquipment { .. }
            | Instruction::ReturnAuraFromGraveyard { .. }) => {
                self.execute_attachment_instruction(frame, i)
            }
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
            Instruction::SacrificeCreature {
                recipient,
                source_block,
            } => {
                if recipient != ActionRecipient::EventPlayer {
                    return Err("unsupported controller sacrifice instruction".into());
                }
                let actor = self.resolution_player(&frame, Some(&recipient))?;
                if !self.is_live(actor) || self.controlled_creatures(actor).is_empty() {
                    self.finish_instruction(
                        frame,
                        InstructionOutcome::Sacrificed {
                            player: actor,
                            movement: None,
                        },
                    )
                } else {
                    self.state.rules.frame = Frame::ResolutionChoice {
                        frame,
                        choice: ResolutionChoice::Sacrifice {
                            player: actor,
                            source_block,
                        },
                    };
                    Ok(())
                }
            }
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
                    self.state.rules.frame = Frame::ResolutionChoice {
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
                    Some(LifeRecipient::EachOpponent) => {
                        return Err("unsupported each-opponent draw".into());
                    }
                    Some(LifeRecipient::EventPlayer) => {
                        let Some(actor) = frame.spell.kind.event_player()? else {
                            return self.finish_instruction(
                                frame,
                                InstructionOutcome::Effect { actual: None },
                            );
                        };
                        actor
                    }
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
                self.draw_one(frame, player, source_block)
            }
            Instruction::DrawDamageAmount { source_block } => {
                self.draw_damage_amount(frame, source_block)
            }
            // CR 603.5/608.2d: offer the option only during resolution, even
            // when the library is empty. Accepting performs the ordinary draw.
            // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
            Instruction::OptionalDrawCards { source_block } => {
                if !self.is_live(player) {
                    return self.finish_instruction(
                        frame,
                        InstructionOutcome::PlayerActionSkipped { player },
                    );
                }
                self.state.rules.frame = Frame::ResolutionChoice {
                    frame,
                    choice: ResolutionChoice::OptionalDraw {
                        player,
                        source_block,
                    },
                };
                Ok(())
            }
            Instruction::DiscardCards {
                recipient,
                count,
                source_block,
            } => {
                let player = self.resolution_player(&frame, recipient.as_ref())?;
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
                    self.state.rules.frame = Frame::ResolutionChoice {
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
    /// One ordinary draw attempt, shared by mandatory and accepted optional
    /// draws. The existing Commander continuation owns the remaining attempt;
    /// recovery must not offer the optional choice again.
    pub(super) fn draw_one(
        &mut self,
        frame: Resolving,
        player: PlayerId,
        source_block: u32,
    ) -> Result<(), String> {
        if !self.is_live(player) {
            return self
                .finish_draw_attempt(frame, InstructionOutcome::PlayerActionSkipped { player });
        }
        if let Some(card) = self
            .state
            .rules
            .libraries
            .get(&player)
            .and_then(|cards| cards.first())
            .copied()
        {
            let object = self
                .object_id(card)
                .ok_or("missing instruction draw source")?;
            if self.state.rules.commanders.history(card).is_some() {
                self.state.rules.frame = Frame::CommanderReturn {
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
                self.finish_draw_attempt(
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
            self.finish_draw_attempt(
                frame,
                InstructionOutcome::Drawn {
                    player,
                    movement: None,
                    failed: true,
                },
            )
        }
    }
    /// CR 120.4b/603.2c: "that many" is the positive final member of this
    /// invocation's actual damage witness, never printed power or life changes.
    pub(super) fn damage_draw_amount(&self, frame: &Resolving) -> Result<Quantity, String> {
        match &frame.spell.kind {
            StackKind::Triggered {
                witness: TriggerEventWitness::DamageDealt { damage, .. },
                ..
            } if damage.amount > Quantity::zero() => Ok(damage.amount.clone()),
            _ => Err("damage-quantity draw requires a positive actual damage witness".into()),
        }
    }
    // CR 121.2/121.4/704.4: one attempt per rule unit, including repeated failed
    // attempts. No SBA, card-count clamp or machine-integer conversion interrupts
    // the exact instruction. Commander replacement consumes just its attempt.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    fn draw_damage_amount(
        &mut self,
        mut frame: Resolving,
        source_block: u32,
    ) -> Result<(), String> {
        self.validate_damage_draw(&frame)?;
        if frame.damage_draw.is_none() {
            frame.damage_draw = Some(DamageDrawProgress {
                cursor: frame.cursor,
                source_block,
                completed: Quantity::zero(),
                remaining: self.damage_draw_amount(&frame)?,
                failed: Quantity::zero(),
            });
        }
        let player = frame.spell.controller;
        self.draw_one(frame, player, source_block)
    }
    /// Finish one ordinary draw attempt. The same owned continuation is used
    /// after a Commander replacement, preserving progress without another draw.
    pub(super) fn finish_draw_attempt(
        &mut self,
        mut frame: Resolving,
        outcome: InstructionOutcome,
    ) -> Result<(), String> {
        if frame.damage_draw.is_none() {
            if matches!(
                self.program_for(&frame.spell)?
                    .instructions
                    .get(frame.cursor as usize),
                Some(Instruction::DrawDamageAmount { .. })
            ) {
                return Err("missing damage-quantity draw progress".into());
            }
            return self.finish_instruction(frame, outcome);
        }
        self.validate_damage_draw(&frame)?;
        let player = frame.spell.controller;
        if matches!(outcome, InstructionOutcome::PlayerActionSkipped { player: actor } if actor == player && !self.is_live(player))
        {
            let progress = frame.damage_draw.take().unwrap();
            return self.finish_instruction(
                frame,
                InstructionOutcome::DamageDrawn {
                    player,
                    attempted: progress.completed,
                    failed: progress.failed,
                },
            );
        }
        let failed = match outcome {
            InstructionOutcome::Drawn {
                player: actor,
                movement,
                failed,
            } if actor == player
                && (if failed {
                    movement.is_none()
                } else {
                    movement.is_some()
                }) =>
            {
                failed
            }
            _ => return Err("invalid damage-quantity draw attempt outcome".into()),
        };
        let progress = frame.damage_draw.as_mut().unwrap();
        progress.completed.add(&Quantity::from(1));
        progress.remaining.subtract(&Quantity::from(1));
        if failed {
            progress.failed.add(&Quantity::from(1));
        }
        if progress.remaining == Quantity::zero() {
            let progress = frame.damage_draw.take().unwrap();
            self.finish_instruction(
                frame,
                InstructionOutcome::DamageDrawn {
                    player,
                    attempted: progress.completed,
                    failed: progress.failed,
                },
            )
        } else {
            self.state.rules.frame = Frame::Resolve { frame };
            Ok(())
        }
    }
    /// Structural guards complement accepted-input replay: neither coherent
    /// counters nor a substituted witness can establish an unrecorded draw.
    fn validate_damage_draw(&self, frame: &Resolving) -> Result<(), String> {
        let program = self.program_for(&frame.spell)?;
        if let Some(progress) = &frame.damage_draw {
            let Some(Instruction::DrawDamageAmount { source_block }) =
                program.instructions.get(frame.cursor as usize)
            else {
                return Err("damage-quantity draw progress on another instruction".into());
            };
            let mut total = progress.completed.clone();
            total.add(&progress.remaining);
            if progress.cursor != frame.cursor
                || progress.source_block != *source_block
                || progress.completed.is_negative()
                || progress.remaining <= Quantity::zero()
                || progress.failed.is_negative()
                || progress.failed > progress.completed
                || total != self.damage_draw_amount(frame)?
            {
                return Err("damage-quantity draw progress invariant".into());
            }
        }
        for (instruction, outcome) in program.instructions.iter().zip(&frame.outcomes) {
            match (instruction, outcome) {
                (
                    Instruction::DrawDamageAmount { .. },
                    InstructionOutcome::DamageDrawn {
                        player,
                        attempted,
                        failed,
                    },
                ) => {
                    let amount = self.damage_draw_amount(frame)?;
                    if *player != frame.spell.controller
                        || attempted.is_negative()
                        || *attempted > amount
                        || failed.is_negative()
                        || failed > attempted
                        || (*attempted != amount && self.is_live(*player))
                    {
                        return Err("damage-quantity draw completed outcome invariant".into());
                    }
                }
                (Instruction::DrawDamageAmount { .. }, _)
                | (_, InstructionOutcome::DamageDrawn { .. }) => {
                    return Err("damage-quantity draw outcome on another instruction".into());
                }
                _ => (),
            }
        }
        Ok(())
    }
    pub(super) fn finish_ability(
        &mut self,
        entry: &StackInstance,
        resolved: bool,
    ) -> Result<(), String> {
        if self
            .state
            .rules
            .stack
            .last()
            .is_some_and(|top| top.id == entry.id)
        {
            self.state.rules.stack.pop();
        } else if self.is_live(entry.controller)
            || self
                .state
                .rules
                .stack
                .iter()
                .any(|item| item.id == entry.id)
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
        self.state.rules.frame = Frame::Priority;
        self.state.rules.priority = self
            .state
            .rules
            .active
            .or_else(|| self.next_live(self.state.rules.turn_owner))
            .ok_or("no live player after ability")?;
        self.stabilize()
    }
    pub(super) fn invocation_lives(&self, entry: &StackInstance) -> bool {
        if entry.kind.is_spell() {
            self.state
                .rules
                .cards
                .get(&entry.source.card)
                .is_some_and(|card| self.is_live(card.owner))
        } else {
            self.is_live(entry.controller)
        }
    }
    pub(super) fn validate_resolving(&self, frame: &Resolving) -> Result<(), String> {
        self.validate_trigger_entry(&frame.spell)?;
        self.validate_mode_binding(&frame.spell)?;
        self.validate_condition_cache(frame)?;
        self.validate_target_facts(frame)?;
        self.validate_set_damage_facts(frame)?;
        self.validate_damage_draw(frame)?;
        let entry = &frame.spell;
        let program = self.program_for(entry)?;
        self.validate_damage_bindings(entry)?;
        if entry.id != frame.stack_id
            || entry.id.0 >= self.state.rules.next_stack_id
            || self.bound_target_count(entry) != program.targets.len()
            || entry.paid.as_ref() != Some(&entry.total_cost)
            || frame.cursor as usize > program.instructions.len()
            || frame.outcomes.len() != frame.cursor as usize
            || frame.legal_targets.len() != program.targets.len()
        {
            return Err("snapshot resolving invocation invariant".into());
        }
        if self
            .state
            .rules
            .stack
            .iter()
            .any(|item| item.id == entry.id)
        {
            if self.top()? != entry {
                return Err("snapshot resolving top binding".into());
            }
        } else if self.invocation_lives(entry) {
            return Err("snapshot detached invocation invariant".into());
        }
        Ok(())
    }
    pub(super) fn revalidate_actions_departure(&mut self) -> Result<(), String> {
        if matches!(
            self.state.rules.frame,
            Frame::ResolutionChoice { .. }
                | Frame::ForcedSacrifice { .. }
                | Frame::OptionalDamage { .. }
                | Frame::SacrificeCost { .. }
        ) && self.state.rules.pending.as_ref().is_some_and(|input| {
            self.validate_response(input.player, &input.response)
                .is_err()
        }) {
            self.state.rules.pending = None;
        }
        Ok(())
    }
    pub(super) fn validate_actions_state(&self) -> Result<(), String> {
        for entry in &self.state.rules.stack {
            let StackKind::Activated {
                ability,
                source_handles,
            } = &entry.kind
            else {
                continue;
            };
            self.validate_activated_entry(entry, *ability, source_handles)?;
        }
        if let Frame::ResolutionChoice { frame, choice } = &self.state.rules.frame {
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
                    Instruction::SacrificeCreature {
                        recipient,
                        source_block: expected,
                    },
                ) => {
                    *recipient == ActionRecipient::EventPlayer
                        && source_block == expected
                        && self
                            .resolution_player(frame, Some(recipient))
                            .is_ok_and(|actor| actor == *player)
                }
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
                        recipient,
                        count: expected,
                        source_block: expected_block,
                    },
                ) => {
                    self.resolution_player(frame, recipient.as_ref())
                        .is_ok_and(|actor| actor == *player)
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
