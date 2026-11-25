use super::*;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "entry", rename_all = "snake_case", deny_unknown_fields)]
pub enum StackKind {
    #[default]
    Spell,
    Triggered {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        match_condition: Option<InterveningCheck>,
        ability: EffectiveTriggerAbility,
        source_handles: Vec<publish::StoredObjectHandle>,
        trigger_id: TriggerId,
        event: TriggerOccurrenceId,
        witness: TriggerEventWitness,
    },
    Activated {
        ability: u32,
        source_handles: Vec<publish::StoredObjectHandle>,
    },
}
impl StackKind {
    pub(super) fn activated_index(&self) -> Option<u32> {
        match self {
            Self::Activated { ability, .. } => Some(*ability),
            _ => None,
        }
    }
    pub fn event_player(&self) -> Result<Option<PlayerId>, String> {
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
    pub fn is_spell(&self) -> bool {
        matches!(self, Self::Spell)
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "choice", rename_all = "snake_case", deny_unknown_fields)]
pub enum ResolutionChoice {
    OptionalEffect {
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
    /// CR 701.23a: `player` chooses the found cards of the admitted search.
    Search {
        player: PlayerId,
        source_block: u32,
    },
    /// CR 701.20e: `player` privately looks at and orders this prefix.
    ReorderLibraryTop {
        player: PlayerId,
        looked: Vec<ObjectId>,
        source_block: u32,
    },
    /// CR 701.22a: `player` looked at `looked` (top first), then chooses the
    /// bottom cards, then the order of the top cards, then of the bottom.
    Scry {
        player: PlayerId,
        looked: Vec<ObjectId>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        bottom: Option<Vec<ObjectId>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        top: Option<Vec<ObjectId>>,
        source_block: u32,
    },
    /// CR 303.4f, 614.1c, 614.12: the found cards of a search onto the
    /// battlefield wait while their controller chooses, in object order,
    /// each found Aura's host and then, entrant by entrant, whether to pay
    /// each entry life cost (`paid`); then they all enter in one batch.
    SearchEntry {
        player: PlayerId,
        found: Vec<ObjectId>,
        /// CR 303.4f: each found Aura's chosen host, Aura first.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        hosts: Vec<(ObjectId, ObjectId)>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        paid: Vec<(ObjectId, bool)>,
        source_block: u32,
    },
    /// CR 701.25a: `player` looked at `looked`, then chooses the graveyard
    /// cards, then the order of the rest on top.
    Surveil {
        player: PlayerId,
        looked: Vec<ObjectId>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        graveyard: Option<Vec<ObjectId>>,
        source_block: u32,
    },
}
impl RulesTxn {
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
    pub(super) fn top_has_target_domain(&self) -> Result<bool, String> {
        let top = self.top()?;
        let source = self.targeting_characteristics(top)?;
        if top.kind.is_spell()
            && top.mode_mask.is_none()
            && let Some(modal) = &self.copiable_definition(top.source)?.modal_spell
        {
            return Ok(self.modal_has_selection(modal, top.controller, &source));
        }
        Ok(self.has_target_domain(self.program_for(top)?, top.controller, &source))
    }

    pub(crate) fn has_target_domain(
        &self,
        program: &SpellProgram,
        player: PlayerId,
        source: &Characteristics,
    ) -> bool {
        if program.optional_target {
            return true;
        }
        match program.targets.first().map(|target| target.domain) {
            Some(TargetDomain::PlayerGraveyardCards { .. }) => false,
            None => true,
            Some(TargetDomain::AnyDamageTarget) => {
                self.live_players()
                    .into_iter()
                    .any(|p| self.targetable_player(p))
                    || self.state.rules.objects.values().any(|o| {
                        self.is_battlefield_creature(o.id)
                            && self.targetable_by(o.id, player)
                            && !self.protected_from(o.id, source)
                    })
            }
            Some(
                domain @ (TargetDomain::AnyPlayer
                | TargetDomain::PlayerOrPlaneswalker
                | TargetDomain::OpponentPlayer),
            ) => !self.eligible_players(domain, player).is_empty(),
            Some(
                TargetDomain::StackSpell
                | TargetDomain::ControlledCreatureSpell
                | TargetDomain::StackActivatedOrTriggeredAbility,
            ) => !self
                .eligible_stack_targets_for(&program.targets[0], player, None)
                .is_empty(),
            Some(
                TargetDomain::BattlefieldPermanent | TargetDomain::ControlledBattlefieldPermanent,
            ) => self.state.rules.objects.values().any(|o| {
                !self.protected_from(o.id, source)
                    && self.permanent_target_legal(o.id, player, &program.targets[0])
            }),
            Some(TargetDomain::OwnGraveyardCreatureCard | TargetDomain::OwnGraveyardCard) => {
                self.state.rules.objects.values().any(|object| {
                    self.graveyard_target_legal_for(object, player, &program.targets[0])
                })
            }
            Some(TargetDomain::BlockingBattlefieldCreature) => {
                self.state.rules.blocking_creatures.iter().any(|o| {
                    self.is_blocking_creature(*o)
                        && self.targetable_by(*o, player)
                        && !self.protected_from(*o, source)
                })
            }
            Some(
                domain @ (TargetDomain::CreatureOrPlaneswalker
                | TargetDomain::BattlefieldCreature
                | TargetDomain::ControlledBattlefieldCreature),
            ) => self.state.rules.objects.values().any(|o| {
                (if domain == TargetDomain::CreatureOrPlaneswalker {
                    self.creature_or_planeswalker(o.id)
                } else {
                    self.is_battlefield_creature(o.id)
                }) && self.targetable_by(o.id, player)
                    && !self.protected_from(o.id, source)
                    && (domain != TargetDomain::ControlledBattlefieldCreature
                        || o.controller == player)
            }),
        }
    }
    pub(crate) fn announced_target_legality(&self, entry: &StackInstance) -> Vec<bool> {
        if !entry.entity_targets.is_empty() {
            entry
                .entity_targets
                .iter()
                .map(|t| self.entity_target_legal(*t, entry))
                .collect()
        } else if !entry.stack_targets.is_empty() {
            entry
                .stack_targets
                .iter()
                .enumerate()
                .map(|(slot, target)| {
                    self.program_for(entry).is_ok_and(|program| {
                        program.targets.get(slot).is_some_and(|spec| {
                            self.eligible_stack_targets_for(spec, entry.controller, Some(entry.id))
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
    pub(crate) fn controlled_creatures(&self, player: PlayerId) -> Vec<ObjectId> {
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
            None | Some(ActionRecipient::Controller) => Ok(self.entry(frame.stack_id)?.controller),
            Some(ActionRecipient::EventPlayer) => {
                let StackKind::Triggered { witness, .. } = &self.entry(frame.stack_id)?.kind else {
                    return Err("unbound damage-recipient action".into());
                };
                match witness {
                    TriggerEventWitness::DamageDealt { damage, .. } => match damage.recipient {
                        DamageRecipient::Player { player } => Ok(player),
                        DamageRecipient::Creature { .. } => {
                            Err("action event requires a player damage recipient".into())
                        }
                    },
                    _ => Err("unbound damage-recipient action".into()),
                }
            }
        }
    }
    fn discard_spec<'a>(
        &self,
        frame: &Resolving,
        instruction: &'a Instruction,
    ) -> Result<(PlayerId, &'a Quantity, u32), String> {
        match instruction {
            kernel!(Discard { player: player @ (you!() | event_player!()), amount: fixed!(count), random: false } @ source_block) =>
            {
                let recipient = player
                    .is_event_player()
                    .then_some(ActionRecipient::EventPlayer);
                Ok((
                    self.resolution_player(frame, recipient.as_ref())?,
                    count,
                    *source_block,
                ))
            }
            kernel!(Discard { player: mtg_contracts::kir::PlayerExpr::EachPlayer | mtg_contracts::kir::PlayerExpr::EachOpponent, amount: fixed!(count), random: false } @ source_block) =>
            {
                let player = frame
                    .group_discard
                    .as_ref()
                    .and_then(|group| group.remaining.first())
                    .ok_or("missing group discard chooser")?;
                Ok((*player, count, *source_block))
            }
            kernel!(Discard { player: target_player!(target_slot), amount: fixed!(count), random: false } @ source_block) =>
            {
                // A sole illegal target ends resolution before instruction dispatch.
                let slot = *target_slot as usize;
                if frame.legal_targets.get(slot) != Some(&true) {
                    return Err("missing legal discard target".into());
                }
                let player = *self
                    .entry(frame.stack_id)?
                    .player_targets
                    .get(slot)
                    .ok_or("missing discard player target")?;
                Ok((player, count, *source_block))
            }
            _ => Err("expected discard instruction".into()),
        }
    }
    pub(crate) fn action_hand(&self, player: PlayerId) -> Vec<ObjectId> {
        self.state
            .rules
            .objects
            .values()
            .filter(|o| o.zone == (Zone::Hand { owner: player }))
            .map(|o| o.id)
            .collect()
    }
    pub(crate) fn validate_activation(
        &self,
        player: PlayerId,
        object: ObjectId,
        ability: u32,
    ) -> Result<(), String> {
        let source = self.current(object)?;
        let definition = self.copiable_definition(object)?;
        let index = ability;
        let ability = definition
            .activated
            .get(ability as usize)
            .ok_or("unknown activated ability")?;
        if !self.is_live(player)
            || match ability.functioning_zone {
                FunctioningZone::Battlefield => {
                    !source.present_on_battlefield() || source.controller != player
                }
                FunctioningZone::Hand => !self.hand_activation_source_payable(player, object),
                FunctioningZone::Graveyard => {
                    !self.graveyard_activation_source_payable(player, object)
                }
                FunctioningZone::Stack | FunctioningZone::Any => true,
            }
        {
            return Err("invalid activated ability source/permission".into());
        }
        if ability.program.timing == SpellTiming::MainPhase && !self.main_phase_action(player) {
            return Err("activation requires sorcery timing".into());
        }
        if ability
            .restriction
            .is_some_and(|r| !self.activation_restriction_allows(player, object, index, r))
        {
            return Err("activation restriction is not met".into());
        }
        if let ActivationCost::TapPayLifeSacrificeSource { life } = &ability.cost {
            self.validate_tap_life_sacrifice_source(player, object, life)?;
        }
        // CR 119.4, 122.1, 602.2b: life can be paid only from a life total at
        // least that large, and only counters the source has can be removed.
        if let ActivationCost::Composite { costs } = &ability.cost
            && (costs.pay_life.is_some() || costs.remove_source_plus_one_counters.is_some())
            && !self.permanent_source_costs_payable(object, player, costs)
        {
            return Err("unpayable life or counter activation cost".into());
        }
        let tap = ability.cost == ActivationCost::TapSource
            || matches!(
                ability.cost,
                ActivationCost::TapPayLifeSacrificeSource { .. }
            )
            || matches!(&ability.cost, ActivationCost::Composite { costs } if costs.tap)
            || matches!(
                &ability.cost,
                ActivationCost::SacrificePermanent { tap: true, .. }
            );
        if tap
            && (source.tapped
                || (self.characteristics(object)?.kind == CardKind::Creature
                    && !source.attack_ready
                    && !self.has_keyword(source.id, KeywordAbility::Haste)))
        {
            return Err("tap ability requires an untapped continuously controlled creature".into());
        }
        if let ActivationCost::SacrificePermanent {
            filter,
            count,
            another,
            ..
        } = &ability.cost
            && usize::try_from(*count).map_or(true, |n| {
                self.sacrifice_cost_candidates(player, filter, another.then_some(object))
                    .len()
                    < n
            })
        {
            return Err("no payable sacrifice permanent".into());
        }
        if !self.has_target_domain(&ability.program, player, self.color_layer(object)?.as_ref()) {
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

    pub(crate) fn actions_request(&self) -> Result<Option<Decision<Rules>>, String> {
        let Frame::ResolutionChoice { choice, frame } = &self.state.rules.frame else {
            return Ok(None);
        };
        Ok(match choice {
            ResolutionChoice::Search { .. }
            | ResolutionChoice::ReorderLibraryTop { .. }
            | ResolutionChoice::Scry { .. }
            | ResolutionChoice::Surveil { .. }
            | ResolutionChoice::SearchEntry { .. } => self.library_request(frame, choice)?,
            ResolutionChoice::OptionalEffect { player, .. } if self.is_live(*player) => {
                Some(self.pick_flags(
                    *player,
                    Reason::OptionalEffect,
                    &[Flag::Accept, Flag::Decline],
                ))
            }
            ResolutionChoice::Sacrifice { player, .. } if self.is_live(*player) => {
                let choices = self.controlled_creatures(*player);
                (!choices.is_empty())
                    .then(|| self.pick_objects(*player, Reason::Sacrifice, choices, 1))
            }
            ResolutionChoice::Discard { player, count, .. } if self.is_live(*player) => {
                let hand = self.action_hand(*player);
                let count = (*count).min(hand.len());
                (!hand.is_empty())
                    .then(|| self.pick_objects(*player, Reason::ResolveDiscard, hand, count))
            }
            _ => None,
        })
    }

    pub(super) fn actions_step(&mut self, frame: Frame) -> Result<(), String> {
        match frame {
            Frame::EntityTargets { cast } => self.bind_entity_targets(cast)?,
            Frame::StackTargets { .. } => {
                return Err("stack targets outside admitted decision algebra".into());
            }
            Frame::PlayerTargets { cast } => {
                let input = self
                    .rules_mut()
                    .pending
                    .take()
                    .ok_or("missing player targets")?;
                let players = input.players()?;
                self.rules_mut()
                    .stack
                    .last_mut()
                    .ok_or("missing player-target stack entry")?
                    .player_targets
                    .extend(players);
                if cast.origin == AnnouncementOrigin::Activated {
                    self.continue_activation_after_targets(cast, input.decision_id)?;
                } else {
                    self.rules_mut().frame = self.announcement_mana_frame(cast)?;
                }
            }
            Frame::ResolutionChoice { mut frame, choice } => {
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
                    ResolutionChoice::Search { .. }
                        | ResolutionChoice::ReorderLibraryTop { .. }
                        | ResolutionChoice::Scry { .. }
                        | ResolutionChoice::Surveil { .. }
                        | ResolutionChoice::SearchEntry { .. }
                ) {
                    return self.library_choice_step(frame, choice);
                }

                let Some(request) = self.actions_request()? else {
                    let outcome = match choice {
                        ResolutionChoice::OptionalEffect { .. }
                        | ResolutionChoice::Search { .. }
                        | ResolutionChoice::ReorderLibraryTop { .. }
                        | ResolutionChoice::Scry { .. }
                        | ResolutionChoice::Surveil { .. }
                        | ResolutionChoice::SearchEntry { .. } => unreachable!(),
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
                    self.rules_mut().pending = None;
                    self.finish_instruction(frame, outcome)?;
                    return Ok(());
                };
                let input = self
                    .rules_mut()
                    .pending
                    .take()
                    .ok_or("missing resolution choice")?;
                let source_block = match choice {
                    ResolutionChoice::OptionalEffect { .. }
                    | ResolutionChoice::Search { .. }
                    | ResolutionChoice::ReorderLibraryTop { .. }
                    | ResolutionChoice::Scry { .. }
                    | ResolutionChoice::Surveil { .. }
                    | ResolutionChoice::SearchEntry { .. } => unreachable!(),
                    ResolutionChoice::Sacrifice { source_block, .. }
                    | ResolutionChoice::Discard { source_block, .. } => source_block,
                };
                let cause = BatchCause::Instruction {
                    stack_id: frame.stack_id,
                    player: self.entry(frame.stack_id)?.controller,
                    source: self.entry(frame.stack_id)?.source,
                    cursor: frame.cursor,
                    source_block,
                };
                match choice {
                    ResolutionChoice::Sacrifice { .. } => {
                        let object = input.object()?;
                        return self.complete_graveyard_group(
                            cause,
                            EventGroup::Occurrence {
                                proposed: ProposedEvent::Sacrifice {
                                    object,
                                    player: request.player,
                                },
                            },
                            GroupFinish::Instruction {
                                frame,
                                result: GroupResult::Sacrifice {
                                    player: request.player,
                                },
                            },
                        );
                    }
                    ResolutionChoice::Discard { .. } => {
                        let objects = input.objects()?;
                        if let Some(group) = &mut frame.group_discard {
                            group.select(request.player, objects)?;
                            self.rules_mut().frame = Frame::Resolve { frame };
                            return Ok(());
                        }
                        return self.complete_resolving_discards(
                            frame,
                            cause,
                            request.player,
                            objects,
                        );
                    }
                    _ => return Err("resolution response invariant".into()),
                };
            }
            _ => return Err("action frame invariant".into()),
        }
        Ok(())
    }
    pub(super) fn valid_discard_selection(
        &self,
        player: PlayerId,
        count: usize,
        objects: &[ObjectId],
    ) -> bool {
        let hand = self.action_hand(player);
        objects.len() == count.min(hand.len())
            && objects.iter().collect::<BTreeSet<_>>().len() == objects.len()
            && objects.iter().all(|o| hand.contains(o))
    }
    pub(super) fn complete_resolving_discards(
        &mut self,
        frame: Resolving,
        cause: BatchCause,
        player: PlayerId,
        objects: Vec<ObjectId>,
    ) -> Result<(), String> {
        if objects.is_empty() {
            return self.finish_instruction(
                frame,
                InstructionOutcome::Discarded {
                    player,
                    movements: vec![],
                },
            );
        }
        let occurrences = objects
            .into_iter()
            .map(|object| ProposedEvent::DiscardCard { object, player })
            .collect();
        self.complete_graveyard_group(
            cause,
            EventGroup::Simultaneous { occurrences },
            GroupFinish::Instruction {
                frame,
                result: GroupResult::Discard { player },
            },
        )
    }

    pub(super) fn finish_instruction(
        &mut self,
        mut frame: Resolving,
        outcome: InstructionOutcome,
    ) -> Result<(), String> {
        if frame.group_discard.is_some() {
            return Err("cannot advance an unfinished group discard".into());
        }
        if frame.random_discard.is_some() {
            return Err("cannot advance an unfinished random discard".into());
        }
        if frame.draw_progress.is_some() || frame.each_player_draw.is_some() {
            return Err("cannot advance an unfinished counted draw".into());
        }
        frame.entrant_draw = None;
        frame.outcomes.push(outcome);
        frame.cursor = frame
            .cursor
            .checked_add(1)
            .ok_or("instruction cursor resource limit")?;
        self.rules_mut().frame = Frame::Resolve { frame };
        Ok(())
    }

    /// CR 701.17a, 701.9a-b, 701.21a: a kernel mill, discard or sacrifice,
    /// performed by the executors whose events, choices and outcomes
    /// recovery re-derives.
    pub(super) fn execute_player_action(
        &mut self,
        frame: Resolving,
        op: Instruction,
    ) -> Result<(), String> {
        match op {
            kernel!(Mill { player, amount } @ source_block) => {
                let recipient = match player {
                    you!() => None,
                    target_player!(target_slot) => Some(LifeRecipient::Target { target_slot }),
                    _ => return Err("kernel mill player has no audited executor".into()),
                };
                let count = match amount {
                    fixed!(value) => value,
                    announced_x!() => self
                        .entry(frame.stack_id)?
                        .x
                        .clone()
                        .ok_or("missing announced X")?,
                    _ => return Err("kernel mill amount has no audited executor".into()),
                };
                self.execute_mill(frame, recipient, count, source_block)
            }
            op @ kernel!(Sacrifice {
                player: mtg_contracts::kir::PlayerExpr::EachPlayer,
                ..
            }) => self.execute_wipes_instruction(frame, op),
            op @ kernel!(Sacrifice {
                player: mtg_contracts::kir::PlayerExpr::EachOpponent,
                ..
            }) => self.execute_sacrifice_instruction(frame, op),
            kernel!(Sacrifice { player: event_player!(), .. } @ source_block) => {
                let actor = self.resolution_player(&frame, Some(&ActionRecipient::EventPlayer))?;
                if !self.is_live(actor) || self.controlled_creatures(actor).is_empty() {
                    self.finish_instruction(
                        frame,
                        InstructionOutcome::Sacrificed {
                            player: actor,
                            movement: None,
                        },
                    )
                } else {
                    self.rules_mut().frame = Frame::ResolutionChoice {
                        frame,
                        choice: ResolutionChoice::Sacrifice {
                            player: actor,
                            source_block,
                        },
                    };
                    Ok(())
                }
            }
            kernel!(Sacrifice { player: target_player!(target_slot), .. } @ source_block) => {
                let target = *self
                    .entry(frame.stack_id)?
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
                    self.rules_mut().frame = Frame::ResolutionChoice {
                        frame,
                        choice: ResolutionChoice::Sacrifice {
                            player: target,
                            source_block,
                        },
                    };
                    Ok(())
                }
            }
            kernel!(Discard { player: player @ (mtg_contracts::kir::PlayerExpr::EachPlayer | mtg_contracts::kir::PlayerExpr::EachOpponent), amount: fixed!(count), random: false } @ source_block) =>
            {
                let scope = if player == mtg_contracts::kir::PlayerExpr::EachPlayer {
                    GroupDiscardScope::EachPlayer
                } else {
                    GroupDiscardScope::EachOpponent
                };
                self.group_discard_step(frame, scope, count, source_block)
            }
            kernel!(Discard {
                player: target_player!(slot),
                ..
            }) if frame.legal_targets.get(slot as usize) != Some(&true) => {
                // CR 608.2b: another legal target can keep the program
                // resolving while this discard's target is illegal.
                self.finish_instruction(frame, InstructionOutcome::Effect { actual: None })
            }
            kernel!(Discard {
                player: target_player!(_),
                random: true,
                ..
            }) => self.random_discard_step(frame),
            op @ (kernel!(Discard {
                player: you!() | event_player!(),
                amount: fixed!(_),
                random: false
            })
            | kernel!(Discard {
                player: target_player!(_),
                amount: fixed!(_),
                random: false
            })) => {
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
                    self.rules_mut().frame = Frame::ResolutionChoice {
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
            _ => Err("kernel player action has no audited executor".into()),
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
                self.rules_mut().frame = Frame::CommanderReturn {
                    object,
                    destination: Zone::Hand { owner: player },
                    continuation: CommanderContinuation::ResolutionDraw {
                        frame,
                        player,
                        source_block,
                    },
                };
                Ok(())
            } else {
                let movement = self.move_object_for(
                    object,
                    Zone::Hand { owner: player },
                    BatchCause::Instruction {
                        stack_id: frame.stack_id,
                        player,
                        source: self.entry(frame.stack_id)?.source,
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
                    stack_id: frame.stack_id,
                    player,
                    source: self.entry(frame.stack_id)?.source,
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
        instruction: &Instruction,
    ) -> Result<Option<(Quantity, u32)>, String> {
        let (amount, source_block) = match instruction {
            // CR 121.2: every kernel draw other than one card by one player
            // is a counted sequence of single draws.
            kernel!(Draw { player, amount } @ source_block) => {
                if mtg_contracts::kir::single_draw(player, amount) {
                    return Ok(None);
                }
                // CR 608.2b: an announced player draws only while that target
                // is legal.
                if let Some(slot) = player.target_slot()
                    && frame.legal_targets.get(slot as usize) != Some(&true)
                {
                    return Err("missing legal counted draw target".into());
                }
                let amount = self.kernel_amount(frame, amount)?;
                if amount.is_negative() {
                    return Err("negative counted draw amount".into());
                }
                (amount, *source_block)
            }

            _ => return Ok(None),
        };
        Ok(Some((amount, source_block)))
    }

    // CR 121.2/121.4/704.4: one attempt per rule unit, including repeated failed
    // attempts. No SBA, card-count clamp or machine-integer conversion interrupts
    // the exact instruction. Commander replacement consumes just its attempt.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    pub(super) fn draw_multiple(&mut self, mut frame: Resolving) -> Result<(), String> {
        self.validate_draw_progress(&frame)?;
        let program = self.program_for(self.entry(frame.stack_id)?)?;
        let instruction = program
            .instructions
            .get(frame.cursor as usize)
            .ok_or("missing counted draw instruction")?;
        let (amount, source_block) = self
            .counted_draw_spec(&frame, instruction)?
            .ok_or("counted draw source invariant")?;
        // CR 121.2c: freeze the live APNAP order once, then perform all of
        // one player's draws before starting the next player's attempts.
        if matches!(
            instruction,
            kernel!(Draw {
                player: mtg_contracts::kir::PlayerExpr::EachPlayer,
                ..
            })
        ) && frame.each_player_draw.is_none()
        {
            frame.each_player_draw = Some(EachPlayerDrawProgress {
                remaining: self.apnap_players(),
                completed: vec![],
            });
        }
        let player = self.draw_player_for_instruction(&frame, instruction)?;
        if amount == Quantity::zero() {
            return self.finish_player_draws(frame, player, Quantity::zero(), Quantity::zero());
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
            let program = self.program_for(self.entry(frame.stack_id)?)?;
            let instruction = program
                .instructions
                .get(frame.cursor as usize)
                .ok_or("missing draw instruction")?;
            if self.counted_draw_spec(&frame, instruction)?.is_some() {
                return Err("missing counted draw progress".into());
            }
            return self.finish_instruction(frame, outcome);
        }
        self.validate_draw_progress(&frame)?;
        let player = self.resolution_draw_player(&frame)?;
        if matches!(outcome, InstructionOutcome::PlayerActionSkipped { player: actor } if actor == player && !self.is_live(player))
        {
            let progress = frame.draw_progress.take().unwrap();
            return self.finish_player_draws(frame, player, progress.completed, progress.failed);
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
            self.finish_player_draws(frame, player, progress.completed, progress.failed)
        } else {
            self.rules_mut().frame = Frame::Resolve { frame };
            Ok(())
        }
    }
    /// Structural guards complement accepted-input replay: neither coherent
    /// counters nor a substituted witness can establish an unrecorded draw.
    fn validate_draw_progress(&self, frame: &Resolving) -> Result<(), String> {
        let program = self.program_for(self.entry(frame.stack_id)?)?;
        if let Some(scope) = &frame.each_player_draw {
            let Some(
                instruction @ kernel!(Draw {
                    player: mtg_contracts::kir::PlayerExpr::EachPlayer,
                    ..
                }),
            ) = program.instructions.get(frame.cursor as usize)
            else {
                return Err("each-player draw progress on another instruction".into());
            };
            let (amount, _) = self
                .counted_draw_spec(frame, instruction)?
                .ok_or("missing each-player draw count")?;
            if scope.remaining.is_empty()
                || (scope.completed.is_empty() && frame.draw_progress.is_none())
            {
                return Err("each-player draw cursor invariant".into());
            }
            self.validate_each_player_draws(&scope.completed, &scope.remaining, &amount)?;
        }
        if let Some(progress) = &frame.draw_progress {
            let instruction = program
                .instructions
                .get(frame.cursor as usize)
                .ok_or("missing counted draw instruction")?;
            let (amount, source_block) = self
                .counted_draw_spec(frame, instruction)?
                .ok_or("counted draw progress on another instruction")?;
            self.draw_player_for_instruction(frame, instruction)?;
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
            let spec = self.counted_draw_spec(frame, instruction)?;
            if matches!(
                instruction,
                kernel!(Draw {
                    player: mtg_contracts::kir::PlayerExpr::EachPlayer,
                    ..
                })
            ) {
                let InstructionOutcome::EachPlayerDrawAttempts { players } = outcome else {
                    return Err("missing each-player draw outcome".into());
                };
                let (amount, _) = spec.ok_or("missing each-player draw count")?;
                self.validate_each_player_draws(players, &[], &amount)?;
                continue;
            }
            match (spec, outcome) {
                (
                    Some((amount, _)),
                    InstructionOutcome::DrawAttempts {
                        player,
                        attempted,
                        failed,
                    },
                ) => {
                    if *player != self.draw_player_for_instruction(frame, instruction)?
                        || attempted.is_negative()
                        || *attempted > amount
                        || failed.is_negative()
                        || failed > attempted
                        || (*attempted != amount && self.is_live(*player))
                    {
                        return Err("counted draw completed outcome invariant".into());
                    }
                }
                (Some(_), _)
                | (
                    None,
                    InstructionOutcome::DrawAttempts { .. }
                    | InstructionOutcome::EachPlayerDrawAttempts { .. },
                ) => {
                    return Err("counted draw outcome on another instruction".into());
                }
                _ => (),
            }
        }
        Ok(())
    }
    pub(super) fn finish_ability(&mut self, id: StackId, resolved: bool) -> Result<(), String> {
        let triggered = matches!(self.entry(id)?.kind, StackKind::Triggered { .. });
        self.remove_resolved_entry(id)?;
        if triggered {
            self.record_event(Event::TriggeredAbilityCompleted {
                stack_id: id,
                resolved,
            })?;
        } else {
            self.record_event(Event::AbilityCompleted { stack_id: id })?;
        }
        self.rules_mut().frame = Frame::Priority;
        self.rules_mut().priority = self
            .state
            .rules
            .active
            .or_else(|| self.next_live(self.state.rules.turn_owner))
            .ok_or("no live player after ability")?;
        Ok(self.stabilize()?)
    }
    pub(super) fn remove_resolved_entry(&mut self, id: StackId) -> Result<(), String> {
        if let Some(index) = self.resolving_entry_index(id)? {
            self.unstack(index)?;
        }
        Ok(())
    }
    /// The stack position of the resolving invocation `id`, with exactly its
    /// completed children above it; `None` once a departure detached it.
    pub(super) fn resolving_entry_index(&self, id: StackId) -> Result<Option<usize>, String> {
        if let Some(index) = self.state.rules.stack.iter().position(|s| s.id == id) {
            if index + 1 != self.state.rules.stack.len() {
                return Err("exact resolving owner completion invariant".into());
            }
            return Ok(Some(index));
        } else if self.invocation_lives(self.entry(id)?) {
            return Err("missing live resolving owner".into());
        }
        Ok(None)
    }
    pub(super) fn invocation_lives(&self, entry: &StackInstance) -> bool {
        if entry.kind.is_spell() {
            self.state
                .rules
                .cards
                .get(&entry.source.card)
                .is_some_and(|card| self.is_live(card.owner) && self.is_live(entry.controller))
        } else {
            self.is_live(entry.controller)
        }
    }
    pub(crate) fn validate_resolving(&self, frame: &Resolving) -> Result<(), String> {
        self.validate_resolving_children(frame, &[])
    }

    pub(super) fn validate_resolving_children(
        &self,
        frame: &Resolving,
        children: &[StackId],
    ) -> Result<(), String> {
        self.validate_trigger_entry(self.entry(frame.stack_id)?)?;
        self.validate_cost_lock(self.entry(frame.stack_id)?)?;
        self.validate_additional_cost_entry(self.entry(frame.stack_id)?)?;
        self.validate_condition_cache(frame)?;
        self.validate_target_facts(frame)?;
        self.validate_set_damage_facts(frame)?;
        self.validate_draw_progress(frame)?;
        self.validate_random_discard_progress(frame)?;
        self.validate_group_discard_progress(frame)?;
        let entry = self.entry(frame.stack_id)?;
        let program = self.program_for(entry)?;
        if frame.entrant_draw.is_some() {
            return Err("unsupported resolving entrant choice".into());
        }
        self.validate_damage_bindings(entry)?;
        if entry.id != frame.stack_id
            || entry.id.0 >= self.state.rules.next_stack_id
            || !self.announced_target_count_valid(program, entry)
            || !self.stack_entry_complete(entry)
            || frame.cursor as usize > program.instructions.len()
            || frame.outcomes.len() != frame.cursor as usize
            || frame.legal_targets.len() != self.bound_target_count(entry)
        {
            return Err("snapshot resolving invocation invariant".into());
        }
        for (instruction, outcome) in program.instructions.iter().zip(&frame.outcomes) {
            self.validate_library_reorder_outcome(frame, Some(instruction), outcome)?;
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
            if self.state.rules.detached.contains_key(&entry.id)
                || !self.state.rules.stack[position + 1..]
                    .iter()
                    .map(|s| s.id)
                    .eq(children.iter().copied())
                || children.first().is_some_and(|child| *child <= entry.id)
            {
                return Err("snapshot resolving owner/child binding".into());
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
                | Frame::SacrificeCost { .. }
        ) && self
            .state
            .rules
            .pending
            .as_ref()
            .is_some_and(|input| self.validate_pending(input).is_err())
        {
            self.rules_mut().pending = None;
        }
        // CR 303.4g, 800.4a: a departure can remove the last host an open
        // Aura host choice offered. With no question left, the entry
        // commits now, leaving that Aura in the library, so a search entry
        // never waits without a question.
        if let Frame::ResolutionChoice {
            frame,
            choice:
                ResolutionChoice::SearchEntry {
                    player,
                    found,
                    hosts,
                    paid,
                    source_block,
                },
        } = &self.state.rules.frame
            && self.is_live(*player)
            && self
                .next_entry_question(*player, found, hosts, paid)?
                .is_none()
        {
            let (frame, player, found, hosts, paid, source_block) = (
                frame.clone(),
                *player,
                found.clone(),
                hosts.clone(),
                paid.clone(),
                *source_block,
            );
            self.rules_mut().pending = None;
            self.continue_search_entry(frame, player, found, hosts, paid, source_block)?;
        }
        Ok(())
    }
    pub(crate) fn validate_actions_state(&self) -> Result<(), String> {
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
                .program_for(self.entry(frame.stack_id)?)?
                .instructions
                .get(frame.cursor as usize)
                .ok_or("snapshot missing choice instruction")?;
            let valid = match (choice, instruction) {
                (
                    ResolutionChoice::Sacrifice {
                        player,
                        source_block,
                    },
                    kernel!(Sacrifice { player: event_player!(), .. } @ expected),
                ) => {
                    source_block == expected
                        && self
                            .resolution_player(frame, Some(&ActionRecipient::EventPlayer))
                            .is_ok_and(|actor| actor == *player)
                }
                (
                    ResolutionChoice::Sacrifice {
                        player,
                        source_block,
                    },
                    kernel!(Sacrifice { player: target_player!(target_slot), .. } @ expected),
                ) => {
                    *source_block == *expected
                        && self
                            .entry(frame.stack_id)?
                            .player_targets
                            .get(*target_slot as usize)
                            == Some(player)
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
                (
                    ResolutionChoice::Search { .. }
                    | ResolutionChoice::ReorderLibraryTop { .. }
                    | ResolutionChoice::Scry { .. }
                    | ResolutionChoice::Surveil { .. }
                    | ResolutionChoice::SearchEntry { .. },
                    _,
                ) => self.valid_library_choice(frame, choice)?,
                _ => self.valid_staples_choice(frame, choice, instruction),
            };
            if !valid {
                return Err("snapshot resolution choice continuation invariant".into());
            }
        }
        Ok(())
    }
}
