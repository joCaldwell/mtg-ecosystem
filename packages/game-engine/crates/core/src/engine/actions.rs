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
        binding: triggers::EventBinding,
    },
    Activated {
        ability: u32,
        source_handles: Vec<publish::StoredObjectHandle>,
        /// CR 602.5: what an "Activate only if" condition read as this
        /// activation began.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        condition: Option<stack_costs::ActivationConditionCheck>,
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
                    | TriggerEventWitness::SpellCast { .. }
                    | TriggerEventWitness::CardDrawn { .. }
                    | TriggerEventWitness::CreatureAttacked { .. }
                    | TriggerEventWitness::AttackerUnblocked { .. }
                    | TriggerEventWitness::BecameBlocked { .. }),
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
    Object {},
    Populate,
    Proliferate,
    OptionalEffect {
        player: PlayerId,
        target: ObjectId,
        source_block: u32,
    },

    TappedReturnPayment {
        player: PlayerId,
        source_block: u32,
    },
    /// CR 701.23a: `player` chooses the found cards of the admitted search.
    Search {
        player: PlayerId,
        source_block: u32,
    },
    /// CR 608.2d: choose which revealed basic land enters tapped.
    SearchSplit {
        player: PlayerId,
        found: [ObjectId; 2],
        source_block: u32,
    },
    /// CR 701.20e: `player` privately looks at and orders this prefix.
    ReorderLibraryTop {
        player: PlayerId,
        looked: Vec<ObjectId>,
        source_block: u32,
    },
    /// CR 701.20e: choose a single card to put into hand.
    LookChooseHand {
        player: PlayerId,
        looked: Vec<ObjectId>,
        source_block: u32,
    },
    /// CR 401.4: order the rest of the private prefix on the bottom.
    LookOrderBottom {
        player: PlayerId,
        looked: Vec<ObjectId>,
        hand: ObjectId,
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
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        command_choices: Vec<(ObjectId, bool)>,
        /// CR 303.4f: each found Aura's chosen host, Aura first.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        hosts: Vec<(ObjectId, ObjectId)>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        paid: Vec<(ObjectId, bool)>,
        /// CR 701.20a, 614.12: then, entrant by entrant, the hand card
        /// revealed for each entry reveal, or `None` when declined.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        revealed: Vec<(ObjectId, Option<ObjectId>)>,
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
        if top.mode_mask.is_none()
            && let Some(modal) = self.modal_declaration(top)?
        {
            let source = self.targeting_characteristics(top)?;
            return Ok(self.modal_has_selection(modal, top.controller, &source, Some(top.source)));
        }
        let program = self.program_for(top)?;
        let source = self.targeting_source(
            top,
            program
                .targets
                .iter()
                .any(mtg_contracts::TargetSpec::requires_source_power),
        )?;
        Ok(self.has_target_domain(program, top.controller, &source, Some(top.source)))
    }

    pub(crate) fn has_target_domain(
        &self,
        program: &SpellProgram,
        player: PlayerId,
        source: &Characteristics,
        source_object: Option<ObjectId>,
    ) -> bool {
        if program.optional_target || program.variable_target_count().is_some() {
            return true;
        }
        // CR 115.3, 601.2c: a counted occurrence needs that many distinct
        // legal objects. Group domains are object domains only.
        if let (Some(count), Some(spec)) = (program.target_group(), program.targets.first()) {
            // The shared legality test the announcement and resolution use.
            let eligible = self
                .state
                .rules
                .objects
                .values()
                .filter(|o| {
                    self.object_target_legal_by(
                        o.id,
                        player,
                        source,
                        source_object,
                        spec,
                        XBound::Unannounced,
                    )
                })
                .count();
            return eligible >= count;
        }
        program.targets.iter().all(|spec| match spec.domain {
            TargetDomain::PlayerGraveyardCards { .. } => false,
            TargetDomain::AnyDamageTarget | TargetDomain::PlayerOrPlaneswalker => {
                self.live_players()
                    .into_iter()
                    .any(|p| self.targetable_player(p))
                    || self.state.rules.objects.values().any(|o| {
                        self.object_target_legal_by(
                            o.id,
                            player,
                            source,
                            source_object,
                            spec,
                            XBound::Unannounced,
                        )
                    })
            }
            domain @ (TargetDomain::AnyPlayer | TargetDomain::OpponentPlayer) => {
                !self.eligible_players(domain, player).is_empty()
            }
            TargetDomain::StackSpell
            | TargetDomain::ControlledCreatureSpell
            | TargetDomain::StackActivatedOrTriggeredAbility => !self
                .eligible_stack_targets_for(spec, player, None)
                .is_empty(),
            _ => self.state.rules.objects.values().any(|o| {
                self.object_target_legal_by(
                    o.id,
                    player,
                    source,
                    source_object,
                    spec,
                    XBound::Unannounced,
                )
            }),
        })
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
                .enumerate()
                .map(|(slot, target)| self.object_target_legal(entry, slot, *target))
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
                // CR 113.10, 305.7: a permanent without its abilities has
                // none to activate.
                FunctioningZone::Battlefield => {
                    !source.present_on_battlefield()
                        || source.controller != player
                        || self.printed_abilities_lost(object)?
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
            .as_ref()
            .is_some_and(|r| !self.activation_restriction_allows(player, object, index, r))
        {
            return Err("activation restriction is not met".into());
        }
        if let ActivationCost::Loyalty { change } = &ability.cost {
            self.loyalty_after_cost(player, object, change)?;
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
                || (self.characteristics(object)?.has_type(CardKind::Creature)
                    && !source.attack_ready
                    && !self.has_keyword(source.id, KeywordAbility::Haste)?))
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
        if let ActivationCost::ChooseObjects(cost) = &ability.cost
            && !self.can_choose_cost(player, object, cost)?
        {
            return Err("no payable chosen-object cost".into());
        }
        let source = self.color_layer(object)?;
        if !match &ability.modal {
            // CR 700.2a: a modal ability needs one selection with legal targets.
            Some(modal) => self.modal_has_selection(modal, player, &source, Some(object)),
            None => self.has_target_domain(&ability.program, player, source.as_ref(), Some(object)),
        } {
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
            ResolutionChoice::Object {} => self.object_choice_request(frame)?,
            ResolutionChoice::Populate | ResolutionChoice::Proliferate => {
                self.keyword_action_request(frame, choice)?
            }
            ResolutionChoice::Search { .. }
            | ResolutionChoice::SearchSplit { .. }
            | ResolutionChoice::ReorderLibraryTop { .. }
            | ResolutionChoice::LookChooseHand { .. }
            | ResolutionChoice::LookOrderBottom { .. }
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
            ResolutionChoice::TappedReturnPayment { player, .. } if self.is_live(*player) => {
                let choices = self.controlled_creatures(*player);
                (!choices.is_empty())
                    .then(|| self.pick_objects(*player, Reason::Sacrifice, choices, 1))
            }

            _ => None,
        })
    }

    pub(super) fn actions_step(&mut self, frame: Frame) -> Result<(), String> {
        match frame {
            Frame::EntityTargets { cast } => self.bind_entity_targets(cast)?,
            Frame::StackTargets { cast } => self.bind_stack_targets(cast)?,
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
            Frame::ResolutionChoice { frame, choice } => {
                if matches!(choice, ResolutionChoice::Object {}) {
                    return self.object_choice_step(frame);
                }
                if matches!(
                    choice,
                    ResolutionChoice::Populate | ResolutionChoice::Proliferate
                ) {
                    return self.keyword_action_step(frame, choice);
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
                    ResolutionChoice::Search { .. }
                        | ResolutionChoice::SearchSplit { .. }
                        | ResolutionChoice::ReorderLibraryTop { .. }
                        | ResolutionChoice::LookChooseHand { .. }
                        | ResolutionChoice::LookOrderBottom { .. }
                        | ResolutionChoice::Scry { .. }
                        | ResolutionChoice::Surveil { .. }
                        | ResolutionChoice::SearchEntry { .. }
                ) {
                    return self.library_choice_step(frame, choice);
                }

                let Some(request) = self.actions_request()? else {
                    let outcome = match choice {
                        ResolutionChoice::Object {}
                        | ResolutionChoice::Populate
                        | ResolutionChoice::Proliferate
                        | ResolutionChoice::OptionalEffect { .. }
                        | ResolutionChoice::Search { .. }
                        | ResolutionChoice::SearchSplit { .. }
                        | ResolutionChoice::ReorderLibraryTop { .. }
                        | ResolutionChoice::LookChooseHand { .. }
                        | ResolutionChoice::LookOrderBottom { .. }
                        | ResolutionChoice::Scry { .. }
                        | ResolutionChoice::Surveil { .. }
                        | ResolutionChoice::SearchEntry { .. } => unreachable!(),
                        ResolutionChoice::TappedReturnPayment { player, .. } => {
                            InstructionOutcome::Sacrificed {
                                player,
                                movement: None,
                            }
                        }
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
                    ResolutionChoice::Object {}
                    | ResolutionChoice::Populate
                    | ResolutionChoice::Proliferate
                    | ResolutionChoice::OptionalEffect { .. }
                    | ResolutionChoice::Search { .. }
                    | ResolutionChoice::SearchSplit { .. }
                    | ResolutionChoice::ReorderLibraryTop { .. }
                    | ResolutionChoice::LookChooseHand { .. }
                    | ResolutionChoice::LookOrderBottom { .. }
                    | ResolutionChoice::Scry { .. }
                    | ResolutionChoice::Surveil { .. }
                    | ResolutionChoice::SearchEntry { .. } => unreachable!(),
                    ResolutionChoice::TappedReturnPayment { source_block, .. } => source_block,
                };
                let cause = BatchCause::Instruction {
                    stack_id: frame.stack_id,
                    player: self.entry(frame.stack_id)?.controller,
                    source: self.entry(frame.stack_id)?.source,
                    cursor: frame.cursor,
                    source_block,
                };
                match choice {
                    ResolutionChoice::TappedReturnPayment { .. } => {
                        let player = request.player;
                        let proposed = ProposedEvent::Sacrifice {
                            player,
                            object: input.object()?,
                        };
                        return self.complete_graveyard_group(
                            cause,
                            EventGroup::Occurrence { proposed },
                            GroupFinish::Instruction {
                                frame,
                                result: GroupResult::Sacrifice { player },
                            },
                        );
                    }
                    _ => return Err("resolution response invariant".into()),
                };
            }
            _ => return Err("action frame invariant".into()),
        }
        Ok(())
    }
    pub(super) fn finish_instruction(
        &mut self,
        mut frame: Resolving,
        outcome: InstructionOutcome,
    ) -> Result<(), String> {
        if frame.random_discard.is_some() {
            return Err("cannot advance an unfinished random discard".into());
        }
        frame.entrant_draw = None;
        frame.fight = None;
        frame.outcomes.push(outcome);
        frame.cursor = frame
            .cursor
            .checked_add(1)
            .ok_or("instruction cursor resource limit")?;
        self.rules_mut().frame = Frame::Resolve { frame };
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
        self.validate_random_discard_progress(frame)?;
        let entry = self.entry(frame.stack_id)?;
        let program = self.program_for(entry)?;
        if frame.entrant_draw.is_some() {
            return Err("unsupported resolving entrant choice".into());
        }
        if frame.fight.is_some()
            && !matches!(
                frame::retained_effect_frame(&self.state.rules.frames, &self.state.rules.frame),
                Frame::DamageEffects { transaction } if matches!(
                    &transaction.continuation, DamageContinuation::Instruction { frame: saved } if saved.as_ref() == frame
                )
            )
        {
            return Err("snapshot fight facts outside their damage frame".into());
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
            self.validate_look_selection_outcome(frame, instruction, outcome)?;
            self.validate_counter_outcome(frame, outcome)?;
            self.validate_myriad_outcome(frame, outcome)?;
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
            Frame::ResolutionChoice { .. } | Frame::SacrificeCost { .. }
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
        // commits now, leaving that Aura in its original zone.
        if let Frame::ResolutionChoice {
            frame,
            choice:
                ResolutionChoice::SearchEntry {
                    player,
                    found,
                    command_choices,
                    hosts,
                    paid,
                    revealed,
                    source_block,
                },
        } = &self.state.rules.frame
            && self.is_live(*player)
            && self
                .card_entry_question(frame, *player, found, hosts, paid, revealed)?
                .is_none()
        {
            let (frame, player, found, command_choices, hosts, paid, revealed, source_block) = (
                frame.clone(),
                *player,
                found.clone(),
                command_choices.clone(),
                hosts.clone(),
                paid.clone(),
                revealed.clone(),
                *source_block,
            );
            self.rules_mut().pending = None;
            self.continue_card_entry(
                frame,
                player,
                found,
                command_choices,
                hosts,
                paid,
                revealed,
                source_block,
            )?;
        }
        Ok(())
    }
    pub(crate) fn validate_actions_state(&self) -> Result<(), String> {
        for entry in &self.state.rules.stack {
            let StackKind::Activated {
                ability,
                source_handles,
                condition,
            } = &entry.kind
            else {
                continue;
            };
            self.validate_activated_entry(entry, *ability, source_handles)?;
            self.validate_activation_condition_check(
                entry.controller,
                self.activated_condition(entry.source, entry.source_face, *ability)?
                    .as_ref(),
                condition.as_ref(),
            )?;
        }
        if let Frame::ResolutionChoice { frame, choice } = &self.state.rules.frame {
            let instruction = self
                .program_for(self.entry(frame.stack_id)?)?
                .instructions
                .get(frame.cursor as usize)
                .ok_or("snapshot missing choice instruction")?;
            let valid = match (choice, instruction) {
                (
                    ResolutionChoice::TappedReturnPayment {
                        player,
                        source_block,
                    },
                    Instruction::Kernel {
                        node: mtg_contracts::kir::Node::SacrificeForTappedReturn { .. },
                        source_block: expected,
                    },
                ) => source_block == expected && *player == self.entry(frame.stack_id)?.controller,
                (ResolutionChoice::Object {}, _) => self.object_choice_spec(frame).is_ok(),
                (
                    ResolutionChoice::Search { .. }
                    | ResolutionChoice::SearchSplit { .. }
                    | ResolutionChoice::ReorderLibraryTop { .. }
                    | ResolutionChoice::LookChooseHand { .. }
                    | ResolutionChoice::LookOrderBottom { .. }
                    | ResolutionChoice::Scry { .. }
                    | ResolutionChoice::Surveil { .. }
                    | ResolutionChoice::SearchEntry { .. },
                    _,
                ) => self.valid_library_choice(frame, choice)?,
                (ResolutionChoice::Populate, kernel!(Populate { .. }))
                | (ResolutionChoice::Proliferate, kernel!(Proliferate { .. })) => true,
                _ => self.valid_staples_choice(frame, choice, instruction),
            };
            if !valid {
                return Err("snapshot resolution choice continuation invariant".into());
            }
        }
        Ok(())
    }
}
