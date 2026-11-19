use super::*;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "entry", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum StackKind {
    #[default]
    Spell,
    Triggered {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        match_condition: Option<InterveningCheck>,
        ability: EffectiveTriggerAbility,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        grant: Option<CascadeGrantProvenance>,
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
    HandReturn {
        player: PlayerId,
        hand: Vec<ObjectId>,
        source_block: u32,
    },
    LibraryOrder {
        player: PlayerId,
        looked: Vec<ObjectId>,
        source_block: u32,
    },
    OptionalShuffle {
        player: PlayerId,
        source_block: u32,
    },
    LibrarySelect {
        player: PlayerId,
        looked: Vec<ObjectId>,
        source_block: u32,
    },
    Surveil {
        player: PlayerId,
        looked: Vec<ObjectId>,
        source_block: u32,
    },
    Scry {
        player: PlayerId,
        looked: Vec<ObjectId>,
        source_block: u32,
    },
    Search {
        player: PlayerId,
        maximum: u32,
        split: bool,
        source_block: u32,
    },
    SearchSplit {
        player: PlayerId,
        objects: Vec<ObjectId>,
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
            Some(
                TargetDomain::BattlefieldPermanent | TargetDomain::ControlledBattlefieldPermanent,
            ) => self
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
    fn discard_spec<'a>(
        &self,
        frame: &Resolving,
        instruction: &'a Instruction,
    ) -> Result<(PlayerId, &'a Quantity, u32), String> {
        match instruction {
            Instruction::DiscardCards {
                recipient,
                count,
                source_block,
            } => Ok((
                self.resolution_player(frame, recipient.as_ref())?,
                count,
                *source_block,
            )),
            Instruction::DiscardTargetPlayer {
                target_slot,
                count,
                source_block,
            } => {
                // A sole illegal target ends resolution before instruction dispatch.
                let slot = *target_slot as usize;
                if frame.legal_targets.get(slot) != Some(&true) {
                    return Err("missing legal discard target".into());
                }
                let player = *frame
                    .spell
                    .player_targets
                    .get(slot)
                    .ok_or("missing discard player target")?;
                Ok((player, count, *source_block))
            }
            _ => Err("expected discard instruction".into()),
        }
    }
    pub(super) fn action_hand(&self, player: PlayerId) -> Vec<ObjectId> {
        self.state
            .rules
            .objects
            .values()
            .filter(|o| o.zone == (Zone::Hand { owner: player }))
            .map(|o| o.id)
            .collect()
    }
    pub(super) fn validate_activation(
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
        if !self.is_live(player)
            || match ability.functioning_zone {
                FunctioningZone::Battlefield => {
                    source.zone != Zone::Battlefield || source.controller != player
                }
                FunctioningZone::Hand => !self.hand_activation_source_payable(player, object),
                FunctioningZone::Stack => true,
            }
        {
            return Err("invalid activated ability source/permission".into());
        }
        if ability.program.timing == SpellTiming::MainPhase && !self.main_phase_action(player) {
            return Err("equip requires sorcery timing".into());
        }
        if let ActivationCost::TapPayLifeSacrificeSource { life } = &ability.cost {
            self.validate_tap_life_sacrifice_source(player, object, life)?;
        }
        let tap = ability.cost == ActivationCost::TapSource
            || matches!(
                ability.cost,
                ActivationCost::TapPayLifeSacrificeSource { .. }
            )
            || matches!(&ability.cost, ActivationCost::Composite { costs } if costs.tap);
        if tap
            && (source.tapped
                || (self.characteristics(object)?.kind == CardKind::Creature
                    && !source.attack_ready
                    && !self.has_keyword(source.id, KeywordAbility::Haste)))
        {
            return Err("tap ability requires an untapped continuously controlled creature".into());
        }
        if let ActivationCost::SacrificePermanent { filter, count, .. } = &ability.cost
            && self.sacrifice_candidates(player, filter).len() < usize::from(*count)
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
        let Frame::ResolutionChoice { frame, choice } = &self.state.rules.frame else {
            return Ok(None);
        };
        let decision_id = self.state.rules.revision;
        Ok(match choice {
            ResolutionChoice::HandReturn { player, .. } if self.is_live(*player) => {
                Some(Request::HandReturn {
                    decision_id,
                    player: *player,
                    count: self.hand_return_count_for_current()?,
                    candidates: self.action_hand(*player),
                })
            }
            ResolutionChoice::LibraryOrder { player, looked, .. } if self.is_live(*player) => {
                Some(Request::LibraryOrder {
                    decision_id,
                    player: *player,
                    candidates: looked.clone(),
                    count: looked.len(),
                })
            }
            ResolutionChoice::LibrarySelect { player, looked, .. } if self.is_live(*player) => {
                Some(Request::LibrarySelect {
                    decision_id,
                    player: *player,
                    candidates: looked.clone(),
                    remainder: self.selection_destination(frame)?,
                })
            }
            ResolutionChoice::Surveil { player, looked, .. } if self.is_live(*player) => {
                Some(Request::Surveil {
                    decision_id,
                    player: *player,
                    candidates: looked.clone(),
                })
            }
            ResolutionChoice::Scry { player, looked, .. } if self.is_live(*player) => {
                Some(Request::Scry {
                    decision_id,
                    player: *player,
                    candidates: looked.clone(),
                })
            }
            ResolutionChoice::Search {
                player, maximum, ..
            } if self.is_live(*player) => Some(Request::SearchLands {
                decision_id,
                player: *player,
                maximum: *maximum,
                candidates: self.search_lands(*player),
            }),
            ResolutionChoice::SearchSplit {
                player, objects, ..
            } if self.is_live(*player) && objects.len() == 2 => {
                Some(Request::SearchLandBattlefield {
                    decision_id,
                    player: *player,
                    candidates: objects.clone(),
                })
            }
            ResolutionChoice::OptionalShuffle { player, .. }
            | ResolutionChoice::OptionalEffect { player, .. }
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
                    ResolutionChoice::OptionalShuffle {
                        player: chooser, ..
                    }
                    | ResolutionChoice::OptionalEffect {
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
                Frame::ResolutionChoice {
                    frame,
                    choice:
                        ResolutionChoice::HandReturn {
                            player: chooser,
                            source_block,
                            ..
                        },
                },
                Response::HandReturn { objects },
            ) => Some(
                self.validate_hand_return(frame, *chooser, *source_block, objects)
                    .and_then(|_| {
                        if player == *chooser {
                            Ok(())
                        } else {
                            Err("wrong hand return chooser".into())
                        }
                    }),
            ),
            (
                Frame::ResolutionChoice {
                    frame,
                    choice:
                        ResolutionChoice::LibraryOrder {
                            player: chooser,
                            looked,
                            source_block,
                        },
                },
                Response::LibraryOrder { objects },
            ) => Some(
                self.validate_library_order_choice(frame, *chooser, looked, *source_block)
                    .and_then(|_| {
                        if player != *chooser {
                            return Err("invalid ordering chooser".into());
                        }
                        self.validate_library_partition(player, looked, objects, &[])
                    }),
            ),
            (
                Frame::ResolutionChoice {
                    frame,
                    choice:
                        ResolutionChoice::LibrarySelect {
                            player: chooser,
                            looked,
                            source_block,
                        },
                },
                Response::LibrarySelect { hand, remainder },
            ) => Some(
                self.validate_selection_choice(frame, *chooser, looked, *source_block)
                    .and_then(|_| {
                        if player != *chooser {
                            return Err("invalid library chooser".into());
                        }
                        self.validate_selection_partition(player, looked, hand, remainder)
                    }),
            ),

            (
                Frame::ResolutionChoice {
                    frame,
                    choice:
                        ResolutionChoice::Surveil {
                            player: chooser,
                            looked,
                            source_block,
                        },
                },
                Response::Surveil { top, graveyard },
            ) => Some(
                self.validate_surveil_choice(frame, *chooser, looked, *source_block)
                    .and_then(|_| {
                        if player != *chooser {
                            return Err("invalid surveil chooser".into());
                        }
                        self.validate_library_partition(player, looked, top, graveyard)
                    }),
            ),
            (
                Frame::ResolutionChoice {
                    frame,
                    choice:
                        ResolutionChoice::Scry {
                            player: chooser,
                            looked,
                            source_block,
                        },
                },
                Response::Scry { top, bottom },
            ) => Some(
                self.validate_scry_choice(frame, *chooser, looked, *source_block)
                    .and_then(|_| {
                        if player != *chooser {
                            return Err("invalid scry chooser".into());
                        }
                        self.validate_library_partition(player, looked, top, bottom)
                    }),
            ),
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
                self.state.rules.frame = self.announcement_mana_frame(cast)?;
            }
            Frame::ResolutionChoice { frame, choice } => {
                if let ResolutionChoice::HandReturn {
                    player,
                    source_block,
                    ..
                } = choice
                {
                    return self.hand_return_step(frame, player, source_block);
                }
                if let ResolutionChoice::LibraryOrder {
                    player,
                    looked,
                    source_block,
                } = choice
                {
                    return self.library_order_step(frame, player, looked, source_block);
                }
                if let ResolutionChoice::OptionalShuffle {
                    player,
                    source_block,
                } = choice
                {
                    return self.optional_shuffle_step(frame, player, source_block);
                }

                if let ResolutionChoice::LibrarySelect {
                    player,
                    looked,
                    source_block,
                } = choice
                {
                    return self.library_selection_step(frame, player, looked, source_block);
                }
                if let ResolutionChoice::Surveil {
                    player,
                    looked,
                    source_block,
                } = choice
                {
                    return self.surveil_choice_step(frame, player, looked, source_block);
                }
                if let ResolutionChoice::Scry {
                    player,
                    looked,
                    source_block,
                } = choice
                {
                    return self.scry_choice_step(frame, player, looked, source_block);
                }
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
                        | ResolutionChoice::SearchSplit { .. }
                        | ResolutionChoice::Shuffle { .. }
                        | ResolutionChoice::OptionalRecursion { .. }
                        | ResolutionChoice::OptionalDraw { .. }
                ) {
                    return self.staples_choice_step(frame, choice);
                }
                let Some(request) = self.actions_request()? else {
                    let outcome = match choice {
                        ResolutionChoice::HandReturn { .. }
                        | ResolutionChoice::LibraryOrder { .. }
                        | ResolutionChoice::OptionalShuffle { .. }
                        | ResolutionChoice::LibrarySelect { .. }
                        | ResolutionChoice::Surveil { .. }
                        | ResolutionChoice::Scry { .. }
                        | ResolutionChoice::OptionalEffect { .. }
                        | ResolutionChoice::Search { .. }
                        | ResolutionChoice::SearchSplit { .. }
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
                    ResolutionChoice::HandReturn { .. }
                    | ResolutionChoice::LibraryOrder { .. }
                    | ResolutionChoice::OptionalShuffle { .. }
                    | ResolutionChoice::LibrarySelect { .. }
                    | ResolutionChoice::Surveil { .. }
                    | ResolutionChoice::Scry { .. }
                    | ResolutionChoice::OptionalEffect { .. }
                    | ResolutionChoice::Search { .. }
                    | ResolutionChoice::SearchSplit { .. }
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
                        self.discard_batch(cause, request.player(), objects)?
                    }
                    _ => return Err("resolution response invariant".into()),
                };
                self.finish_instruction(frame, outcome)?;
            }
            _ => return Err("action frame invariant".into()),
        }
        Ok(())
    }
    pub(super) fn discard_batch(
        &mut self,
        cause: BatchCause,
        player: PlayerId,
        objects: Vec<ObjectId>,
    ) -> Result<InstructionOutcome, String> {
        let movements = if objects.is_empty() {
            vec![]
        } else {
            let result = self.commit_batch(
                cause,
                EventGroup::Simultaneous {
                    occurrences: objects
                        .into_iter()
                        .map(|object| ProposedEvent::DiscardCard { object, player })
                        .collect(),
                },
            )?;
            result
                .occurrences
                .iter()
                .map(|r| match &r.actual {
                    EventOutcome::CardDiscarded { movement, .. } => Ok(movement.clone()),
                    _ => Err("missing simultaneous discard outcome".to_string()),
                })
                .collect::<Result<Vec<_>, _>>()?
        };
        Ok(InstructionOutcome::Discarded { player, movements })
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
        if frame.random_discard.is_some() {
            return Err("cannot advance an unfinished random discard".into());
        }
        if frame.draw_progress.is_some() {
            return Err("cannot advance an unfinished counted draw".into());
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
            Instruction::Cascade { source_block, .. } => self.begin_cascade(frame, source_block),
            Instruction::PutHandOnLibrary {
                count,
                source_block,
            } => self.begin_hand_return(frame, count, source_block),
            Instruction::DrawControllerCards { .. } => self.draw_multiple(frame),
            Instruction::OrderLibraryTop {
                amount,
                source_block,
            } => self.begin_library_order(frame, amount, source_block),
            Instruction::OptionalShuffleLibrary { source_block } => {
                self.begin_optional_shuffle(frame, source_block)
            }
            Instruction::SelectLibraryCard {
                amount,
                source_block,
                ..
            } => self.begin_library_selection(frame, amount, source_block),
            Instruction::MillCards {
                recipient,
                count,
                source_block,
            } => self.execute_mill(frame, recipient, count, source_block),
            Instruction::MillAnnouncedX {
                target_slot,
                source_block,
            } => {
                let count = frame.spell.x.clone().ok_or("missing announced X")?;
                self.execute_mill(
                    frame,
                    Some(LifeRecipient::Target { target_slot }),
                    count,
                    source_block,
                )
            }
            Instruction::Scry {
                amount,
                source_block,
            } => self.begin_scry(frame, amount, source_block),
            Instruction::Surveil {
                amount,
                source_block,
            } => self.begin_surveil(frame, amount, source_block),
            i @ (Instruction::MoveSet { .. }
            | Instruction::DestroySet { .. }
            | Instruction::PumpSet { .. }
            | Instruction::GrantKeywordsSet { .. }
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
            i @ (Instruction::ReturnEntryTargetCreature { .. }
            | Instruction::DestroyPermanent { .. }
            | Instruction::ExilePermanent { .. }
            | Instruction::UntapPermanent { .. }
            | Instruction::TapPermanent { .. }
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
            | Instruction::SearchBasicLands { .. }
            | Instruction::SearchBasicLandsSplit { .. }
            | Instruction::SearchTypedLands { .. }) => self.execute_staples_instruction(frame, op),
            op @ (Instruction::CreateCreatureTokens { .. }
            | Instruction::PutPlusOneCounters { .. }
            | Instruction::PutSourcePlusOneCountersForCastColors { .. }) => {
                self.execute_token_instruction(frame, op)
            }
            op @ (Instruction::GrantKeywords { .. }
            | Instruction::ModifyPowerToughness { .. }
            | Instruction::PreventCombatDamage { .. }
            | Instruction::GrantLandPlaysThisTurn { .. }
            | Instruction::PreventNextDamage { .. }) => self.execute_effect_instruction(frame, op),
            Instruction::ChangeLifeAnnouncedX {
                recipient,
                gain,
                source_block,
            } => {
                let amount = frame.spell.x.clone().ok_or("missing announced X")?;
                if amount == Quantity::zero() {
                    return self
                        .finish_instruction(frame, InstructionOutcome::Life { actual: None });
                }
                let op = if gain {
                    Instruction::GainLife {
                        recipient,
                        amount,
                        source_block,
                    }
                } else {
                    Instruction::LoseLife {
                        recipient,
                        amount,
                        source_block,
                    }
                };
                self.execute_damage_life_instruction(frame, op)
            }
            op @ (Instruction::DealDamage { .. }
            | Instruction::DealDamageToController { .. }
            | Instruction::DealDamageAnnouncedX { .. }
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
                    return self.draw_multiple(frame);
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
            Instruction::DrawCardsIfAdditionalCost { cost_identity, .. } => {
                if !self.additional_cost_was_selected(&frame.spell, &cost_identity)? {
                    return self
                        .finish_instruction(frame, InstructionOutcome::Effect { actual: None });
                }
                self.draw_multiple(frame)
            }
            Instruction::DrawDamageAmount { .. } | Instruction::DrawAnnouncedX { .. } => {
                self.draw_multiple(frame)
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
            Instruction::DiscardTargetPlayerAtRandom { .. } => self.random_discard_step(frame),
            op @ (Instruction::DiscardCards { .. } | Instruction::DiscardTargetPlayer { .. }) => {
                let (player, count, source_block) = self.discard_spec(&frame, &op)?;
                if count.is_negative() {
                    return Err("negative discard count".into());
                }
                if !self.is_live(player) {
                    return self.finish_instruction(
                        frame,
                        InstructionOutcome::PlayerActionSkipped { player },
                    );
                }
                let actual_count = self.bounded_hand_count(player, count);
                if actual_count == 0 {
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
                            count: actual_count,
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
    fn counted_draw_spec(
        &self,
        frame: &Resolving,
        program: &SpellProgram,
        instruction: &Instruction,
    ) -> Result<Option<(Quantity, u32)>, String> {
        let (amount, source_block) = match instruction {
            Instruction::DrawControllerCards {
                count,
                source_block,
            } => {
                if count.is_negative() {
                    return Err("negative counted draw amount".into());
                }
                (count.clone(), *source_block)
            }
            Instruction::DrawAnnouncedX {
                recipient,
                source_block,
            } => {
                if let LifeRecipient::Target { target_slot } = recipient
                    && frame.legal_targets.get(*target_slot as usize) != Some(&true)
                {
                    return Err("missing legal announced-X draw target".into());
                }
                (
                    frame.spell.x.clone().ok_or("missing announced X")?,
                    *source_block,
                )
            }
            Instruction::DrawDamageAmount { source_block } => {
                (self.damage_draw_amount(frame)?, *source_block)
            }
            Instruction::DrawCardsIfAdditionalCost {
                cost_identity,
                count,
                source_block,
            } => {
                if !self.additional_cost_was_selected(&frame.spell, cost_identity)? {
                    return Ok(None);
                }
                (count.clone(), *source_block)
            }
            Instruction::DrawCards {
                count,
                source_block,
                ..
            } if *count != Quantity::from(1) => {
                if !targeted_counted_draw_program(program) {
                    return Err("unsupported draw count".into());
                }
                if frame.legal_targets.first() != Some(&true) {
                    return Err("missing legal counted draw target".into());
                }
                (count.clone(), *source_block)
            }
            _ => return Ok(None),
        };
        Ok(Some((amount, source_block)))
    }
    /// CR 120.4b/603.2c: "that many" is the positive final member of this
    /// invocation's actual damage witness, never printed power or life changes.
    pub(super) fn damage_draw_amount(&self, frame: &Resolving) -> Result<Quantity, String> {
        if let Some(Instruction::DrawCardsIfAdditionalCost {
            cost_identity,
            count,
            ..
        }) = self
            .program_for(&frame.spell)?
            .instructions
            .get(frame.cursor as usize)
        {
            return if self.additional_cost_was_selected(&frame.spell, cost_identity)? {
                Ok(count.clone())
            } else {
                Err("unselected optional-cost draw progress".into())
            };
        }
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
    fn draw_multiple(&mut self, mut frame: Resolving) -> Result<(), String> {
        self.validate_draw_progress(&frame)?;
        let program = self.program_for(&frame.spell)?;
        let instruction = program
            .instructions
            .get(frame.cursor as usize)
            .ok_or("missing counted draw instruction")?;
        let (amount, source_block) = self
            .counted_draw_spec(&frame, program, instruction)?
            .ok_or("counted draw source invariant")?;
        let player = Self::draw_player_for_instruction(&frame, instruction)?;
        if amount == Quantity::zero() {
            return self.finish_instruction(
                frame,
                InstructionOutcome::DrawAttempts {
                    player,
                    attempted: Quantity::zero(),
                    failed: Quantity::zero(),
                },
            );
        }
        if frame.draw_progress.is_none() {
            frame.draw_progress = Some(DrawProgress {
                cursor: frame.cursor,
                source_block,
                completed: Quantity::zero(),
                remaining: amount,
                failed: Quantity::zero(),
            });
        }
        self.draw_one(frame, player, source_block)
    }
    /// Finish one ordinary draw attempt. The same owned continuation is used
    /// after a Commander replacement, preserving progress without another draw.
    pub(super) fn finish_draw_attempt(
        &mut self,
        mut frame: Resolving,
        outcome: InstructionOutcome,
    ) -> Result<(), String> {
        if frame.draw_progress.is_none() {
            let program = self.program_for(&frame.spell)?;
            let instruction = program
                .instructions
                .get(frame.cursor as usize)
                .ok_or("missing draw instruction")?;
            if self
                .counted_draw_spec(&frame, program, instruction)?
                .is_some()
            {
                return Err("missing counted draw progress".into());
            }
            return self.finish_instruction(frame, outcome);
        }
        self.validate_draw_progress(&frame)?;
        let player = self.resolution_draw_player(&frame)?;
        if matches!(outcome, InstructionOutcome::PlayerActionSkipped { player: actor } if actor == player && !self.is_live(player))
        {
            let progress = frame.draw_progress.take().unwrap();
            return self.finish_instruction(
                frame,
                InstructionOutcome::DrawAttempts {
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
            _ => return Err("invalid counted draw attempt outcome".into()),
        };
        let progress = frame.draw_progress.as_mut().unwrap();
        progress.completed.add(&Quantity::from(1));
        progress.remaining.subtract(&Quantity::from(1));
        if failed {
            progress.failed.add(&Quantity::from(1));
        }
        if progress.remaining == Quantity::zero() {
            let progress = frame.draw_progress.take().unwrap();
            self.finish_instruction(
                frame,
                InstructionOutcome::DrawAttempts {
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
    fn validate_draw_progress(&self, frame: &Resolving) -> Result<(), String> {
        let program = self.program_for(&frame.spell)?;
        if let Some(progress) = &frame.draw_progress {
            let instruction = program
                .instructions
                .get(frame.cursor as usize)
                .ok_or("missing counted draw instruction")?;
            let (amount, source_block) = self
                .counted_draw_spec(frame, program, instruction)?
                .ok_or("counted draw progress on another instruction")?;
            Self::draw_player_for_instruction(frame, instruction)?;
            let mut total = progress.completed.clone();
            total.add(&progress.remaining);
            if progress.cursor != frame.cursor
                || progress.source_block != source_block
                || progress.completed.is_negative()
                || progress.remaining <= Quantity::zero()
                || progress.failed.is_negative()
                || progress.failed > progress.completed
                || total != amount
            {
                return Err("counted draw progress invariant".into());
            }
        }
        for (instruction, outcome) in program.instructions.iter().zip(&frame.outcomes) {
            match (
                self.counted_draw_spec(frame, program, instruction)?,
                outcome,
            ) {
                (
                    Some((amount, _)),
                    InstructionOutcome::DrawAttempts {
                        player,
                        attempted,
                        failed,
                    },
                ) => {
                    if *player != Self::draw_player_for_instruction(frame, instruction)?
                        || attempted.is_negative()
                        || *attempted > amount
                        || failed.is_negative()
                        || failed > attempted
                        || (*attempted != amount && self.is_live(*player))
                    {
                        return Err("counted draw completed outcome invariant".into());
                    }
                }
                (Some(_), _) | (None, InstructionOutcome::DrawAttempts { .. }) => {
                    return Err("counted draw outcome on another instruction".into());
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
        self.remove_resolved_entry(entry)?;
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
    pub(super) fn remove_resolved_entry(&mut self, entry: &StackInstance) -> Result<(), String> {
        if let Some(index) = self.state.rules.stack.iter().position(|s| s.id == entry.id) {
            let child = self.resolution_child(entry.id);
            if self.state.rules.stack[index] != *entry
                || match child {
                    None => index + 1 != self.state.rules.stack.len(),
                    Some(child) => {
                        index + 2 != self.state.rules.stack.len()
                            || self.top()?.id != child
                            || self.top()?.paid.is_none()
                    }
                }
            {
                return Err("exact resolving owner completion invariant".into());
            }
            self.state.rules.stack.remove(index);
        } else if self.invocation_lives(entry) {
            return Err("missing live resolving owner".into());
        }
        Ok(())
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
        self.validate_resolving_owned(frame, None)
    }
    pub(super) fn validate_resolving_owned(
        &self,
        frame: &Resolving,
        child: Option<StackId>,
    ) -> Result<(), String> {
        self.validate_trigger_entry(&frame.spell)?;
        self.validate_cost_lock(&frame.spell)?;
        self.validate_additional_cost_entry(&frame.spell)?;
        self.validate_mode_binding(&frame.spell)?;
        self.validate_condition_cache(frame)?;
        self.validate_target_facts(frame)?;
        self.validate_set_damage_facts(frame)?;
        self.validate_draw_progress(frame)?;
        self.validate_random_discard_progress(frame)?;
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
            let position = self
                .state
                .rules
                .stack
                .iter()
                .position(|s| s.id == entry.id)
                .ok_or("missing resolving entry")?;
            if self.state.rules.stack.get(position) != Some(entry)
                || match child {
                    None => position + 1 != self.state.rules.stack.len(),
                    Some(child) => {
                        position + 2 != self.state.rules.stack.len()
                            || self.top()?.id != child
                            || child <= entry.id
                    }
                }
            {
                return Err("snapshot resolving owner/child binding".into());
            }
        } else if self.invocation_lives(entry) {
            return Err("snapshot detached invocation invariant".into());
        }
        Ok(())
    }
    pub(super) fn revalidate_actions_departure(&mut self) -> Result<(), String> {
        if let Frame::ResolutionChoice {
            frame,
            choice: ResolutionChoice::OptionalEffect { target, .. },
        } = &self.state.rules.frame
            && self.current(*target).is_err()
            && matches!(
                self.program_for(&frame.spell)?
                    .instructions
                    .get(frame.cursor as usize),
                Some(Instruction::ReturnEntryTargetCreature { .. })
            )
        {
            let outcome = InstructionOutcome::ObjectNoLongerPresent { target: *target };
            let frame = frame.clone();
            self.state.rules.pending = None;
            return self.finish_instruction(frame, outcome);
        }
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
                    ResolutionChoice::HandReturn {
                        player,
                        hand,
                        source_block,
                    },
                    Instruction::PutHandOnLibrary { .. },
                ) => {
                    self.validate_hand_return_source(frame, *player, *source_block)
                        .is_ok()
                        && (!self.is_live(*player) || *hand == self.action_hand(*player))
                }
                (
                    ResolutionChoice::LibraryOrder {
                        player,
                        looked,
                        source_block,
                    },
                    Instruction::OrderLibraryTop { .. },
                ) => self
                    .validate_library_order_choice(frame, *player, looked, *source_block)
                    .is_ok(),
                (
                    ResolutionChoice::OptionalShuffle {
                        player,
                        source_block,
                    },
                    Instruction::OptionalShuffleLibrary { source_block: b },
                ) => *player == frame.spell.controller && source_block == b,

                (
                    ResolutionChoice::LibrarySelect {
                        player,
                        looked,
                        source_block,
                    },
                    Instruction::SelectLibraryCard { .. },
                ) => self
                    .validate_selection_choice(frame, *player, looked, *source_block)
                    .is_ok(),
                (
                    ResolutionChoice::Surveil {
                        player,
                        looked,
                        source_block,
                    },
                    Instruction::Surveil { .. },
                ) => self
                    .validate_surveil_choice(frame, *player, looked, *source_block)
                    .is_ok(),
                (
                    ResolutionChoice::Scry {
                        player,
                        looked,
                        source_block,
                    },
                    Instruction::Scry { .. },
                ) => self
                    .validate_scry_choice(frame, *player, looked, *source_block)
                    .is_ok(),
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
                    op,
                ) => self
                    .discard_spec(frame, op)
                    .is_ok_and(|(actor, expected, block)| {
                        actor == *player
                            && *count > 0
                            && (!self.is_live(*player)
                                || *count == self.bounded_hand_count(*player, expected))
                            && Quantity::from(*count as i64) <= *expected
                            && *source_block == block
                    }),
                _ => self.valid_staples_choice(frame, choice, instruction),
            };
            if !valid {
                return Err("snapshot resolution choice continuation invariant".into());
            }
        }
        Ok(())
    }
}
