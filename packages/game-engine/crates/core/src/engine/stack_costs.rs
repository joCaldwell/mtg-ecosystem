use super::*;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnnouncementOrigin {
    #[default]
    Spell,
    Activated,
}
impl AnnouncementOrigin {
    pub(super) fn is_spell(&self) -> bool {
        matches!(self, Self::Spell)
    }
}

impl RulesTxn {
    // CR602.2/702.29a: a hand card's owner activates it. A stale battlefield
    // controller never supplies (or removes) permission outside battlefield.
    pub(super) fn hand_activation_source_payable(
        &self,
        player: PlayerId,
        source: ObjectId,
    ) -> bool {
        self.is_live(player)
            && self
                .current(source)
                .is_ok_and(|o| o.zone == (Zone::Hand { owner: player }))
            && self
                .state
                .rules
                .cards
                .get(&source.card)
                .is_some_and(|c| c.owner == player)
    }
    // CR 118.3/119.4/602.2: payer is the current controller, and paying
    // exactly all remaining life is legal. Zero is payable at any life total.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    pub(crate) fn validate_tap_life_sacrifice_source(
        &self,
        player: PlayerId,
        source: ObjectId,
        life: &Quantity,
    ) -> Result<(), String> {
        let object = self.current(source)?;
        if !object.present_on_battlefield()
            || object.controller != player
            || !self.is_live(player)
            || object.tapped
            || !self.state.rules.players[player.0 as usize].can_pay_life(life)
        {
            return Err("unpayable source tap, life and sacrifice cost".into());
        }
        Ok(())
    }
    fn pay_tap_life_sacrifice_source(
        &mut self,
        player: PlayerId,
        source: ObjectId,
        life: &Quantity,
        cause: BatchCause,
    ) -> Result<(), String> {
        self.validate_tap_life_sacrifice_source(player, source, life)?;
        let mut children = vec![EventGroup::Occurrence {
            proposed: ProposedEvent::Tap { object: source },
        }];
        // Paying life is loss of life, not damage. Reuse the existing exact
        // life-change occurrence; zero costs deliberately emit no life event.
        if *life > Quantity::zero() {
            children.push(EventGroup::Occurrence {
                proposed: ProposedEvent::LoseLife {
                    player,
                    amount: life.clone(),
                },
            });
        }
        children.push(EventGroup::Occurrence {
            proposed: ProposedEvent::Sacrifice {
                object: source,
                player,
            },
        });
        // One ordered batch inside the staged rule step: no SBA/priority or
        // observable partial tap/debit before the complete cost has committed.
        self.commit_batch(cause, EventGroup::Ordered { children })?;
        Ok(())
    }
    pub(crate) fn announce_ability(
        &mut self,
        player: PlayerId,
        decision_id: u64,
        object: ObjectId,
        ability: u32,
    ) -> Result<(), String> {
        // CR602.2a/701.20a: reveal only after the ordinary priority authority
        // and exact source/ability permission have succeeded. This precedes
        // every opponent handle allocation; action queries allocate nothing.
        if !matches!(self.state.rules.frame, Frame::Priority) || self.state.rules.priority != player
        {
            return Err("ability proposal requires its ordinary priority owner".into());
        }
        self.validate_activation(player, object, ability)?;
        let cost = self
            .copiable_definition(object)?
            .activated
            .get(ability as usize)
            .ok_or("missing activated cost")?
            .cost
            .clone();
        let cast_from = self.current(object)?.zone;
        let source_handles = self.capture_trigger_source_handles(object)?;
        let id = StackId(self.state.rules.next_stack_id);
        self.rules_mut().next_stack_id = self
            .state
            .rules
            .next_stack_id
            .checked_add(1)
            .ok_or("stack ID resource limit")?;
        let total_cost = match &cost {
            ActivationCost::SacrificePermanent { mana, .. } => {
                mana.clone().unwrap_or_else(zero_mana_cost)
            }
            ActivationCost::Mana { cost } => cost.clone(),
            ActivationCost::DiscardSource { mana } => mana.clone(),
            ActivationCost::Composite { costs } => {
                costs.mana.clone().unwrap_or_else(zero_mana_cost)
            }
            _ => ManaCost {
                generic: Quantity::zero(),
                colored: vec![],
            },
        };
        let chosen = matches!(cost, ActivationCost::SacrificePermanent { .. });
        let mana = matches!(
            cost,
            ActivationCost::Mana { .. }
                | ActivationCost::DiscardSource { .. }
                | ActivationCost::SacrificePermanent { mana: Some(_), .. }
        ) || matches!(&cost, ActivationCost::Composite { costs } if costs.mana.is_some());
        let modal = !self.copiable_definition(object)?.activated[ability as usize]
            .program
            .modes
            .is_empty();
        let targeted = !self.copiable_definition(object)?.activated[ability as usize]
            .program
            .targets
            .is_empty();
        let entry = StackInstance {
            hybrid_colors: vec![],
            cost_lock: None,
            selected_additional_costs: None,
            alternative_cost: None,
            hand_cast_permission: None,
            x: None,
            death_controller: None,
            sacrificed: None,
            sacrificed_set: vec![],
            mode_subset: None,
            mode: None,
            effect_source_handles: vec![],
            source_facts: { Some(self.object_before(object)?) },
            entity_targets: vec![],
            announced_entity_targets: vec![],
            kind: StackKind::Activated {
                ability,
                source_handles,
            },
            stack_targets: vec![],
            announced_stack_targets: vec![],
            player_targets: vec![],
            id,
            source: object,
            controller: player,
            targets: vec![],
            announced_targets: vec![vec![]; 4],
            paid: (!mana && !targeted && !chosen).then_some(total_cost.clone()),
            total_cost,
            cast_from,
        };
        self.rules_mut().stack.push(entry);
        if modal || targeted {
            if cost != ActivationCost::TapSource
                && !matches!(
                    cost,
                    ActivationCost::Mana { .. }
                        | ActivationCost::Composite { .. }
                        | ActivationCost::SacrificePermanent { .. }
                        | ActivationCost::SacrificeSource
                )
            {
                return Err("unsupported targeted activation cost".into());
            }
            let cast = Casting {
                completion: CastCompletion::Priority,
                sacrifices: vec![],
                origin: AnnouncementOrigin::Activated,
                spending: None,
                spending_restricted: vec![],
                stack_id: id,
                original_object: object,
                prior_passes: self.state.rules.passes.clone(),
                mana_activations: vec![],
                undo_mana: false,
            };
            self.rules_mut().frame = if modal {
                Frame::Modes { cast }
            } else {
                match self.copiable_definition(object)?.activated[ability as usize]
                    .program
                    .targets[0]
                    .domain
                {
                    TargetDomain::AnyPlayer => Frame::PlayerTargets { cast },
                    TargetDomain::StackSpell | TargetDomain::StackActivatedOrTriggeredAbility => {
                        Frame::StackTargets { cast }
                    }
                    TargetDomain::ControlledBattlefieldCreature
                    | TargetDomain::BattlefieldCreature
                    | TargetDomain::BattlefieldPermanent => Frame::Targets { cast },
                    _ => Frame::EntityTargets { cast },
                }
            };
        } else if mana {
            self.rules_mut().frame = Frame::Mana {
                cast: Casting {
                    completion: CastCompletion::Priority,
                    sacrifices: vec![],
                    origin: AnnouncementOrigin::Activated,
                    spending: None,
                    spending_restricted: vec![],
                    stack_id: id,
                    original_object: object,
                    prior_passes: self.state.rules.passes.clone(),
                    mana_activations: vec![],
                    undo_mana: false,
                },
            };
        } else if chosen {
            // CR 602.2b/601.2g-h: no mana opportunity for a cost with no mana.
            self.rules_mut().frame = Frame::SacrificeCost {
                cast: Casting {
                    completion: CastCompletion::Priority,
                    sacrifices: vec![],
                    origin: AnnouncementOrigin::Activated,
                    spending: Some(BTreeMap::new()),
                    spending_restricted: vec![],
                    stack_id: id,
                    original_object: object,
                    prior_passes: self.state.rules.passes.clone(),
                    mana_activations: vec![],
                    undo_mana: false,
                },
            };
        } else {
            if let ActivationCost::TapPayLifeSacrificeSource { life } = &cost {
                self.pay_tap_life_sacrifice_source(
                    player,
                    object,
                    life,
                    BatchCause::AbilityActivation {
                        stack_id: id,
                        player,
                        source: object,
                        decision_id,
                        ability,
                    },
                )?;
            } else if let ActivationCost::Composite { costs } = &cost {
                self.pay_permanent_source_costs(
                    object,
                    player,
                    costs,
                    BatchCause::AbilityActivation {
                        stack_id: id,
                        player,
                        source: object,
                        decision_id,
                        ability,
                    },
                )?;
            } else {
                let proposed = match cost {
                    ActivationCost::SacrificeSource => ProposedEvent::Sacrifice { object, player },
                    ActivationCost::TapSource => ProposedEvent::Tap { object },
                    ActivationCost::TapPayLifeSacrificeSource { .. }
                    | ActivationCost::Composite { .. }
                    | ActivationCost::Mana { .. }
                    | ActivationCost::DiscardSource { .. }
                    | ActivationCost::SacrificePermanent { .. } => {
                        return Err("activated cost branch invariant".into());
                    }
                };
                self.commit(
                    proposed,
                    BatchCause::AbilityActivation {
                        stack_id: id,
                        player,
                        source: object,
                        decision_id,
                        ability,
                    },
                )?;
            }
            self.record_event(Event::AbilityActivated {
                stack_id: id,
                source: object,
                player,
                ability,
            })?;
            self.rules_mut().priority = player;
            self.stabilize()?;
        }
        self.rules_mut().passes.clear();
        Ok(())
    }
    pub(super) fn validate_activated_entry(
        &self,
        entry: &StackInstance,
        ability: u32,
        source_handles: &[ObjectHandle],
    ) -> Result<(), String> {
        let definition = self.activation_definition(entry.source)?;
        let activated = definition
            .activated
            .get(ability as usize)
            .ok_or("snapshot missing activated definition")?;
        let expected = match &activated.cost {
            ActivationCost::SacrificePermanent { mana, .. } => {
                mana.clone().unwrap_or_else(zero_mana_cost)
            }
            ActivationCost::Mana { cost } => cost.clone(),
            ActivationCost::DiscardSource { mana } => mana.clone(),
            ActivationCost::Composite { costs } => {
                costs.mana.clone().unwrap_or_else(zero_mana_cost)
            }
            _ => ManaCost {
                generic: Quantity::zero(),
                colored: vec![],
            },
        };
        let program = self.program_for(entry)?;
        let announcing = match self.underlying_mana_frame(&self.state.rules.frame) {
            Frame::Modes { cast }
            | Frame::Targets { cast }
            | Frame::PlayerTargets { cast }
            | Frame::EntityTargets { cast }
            | Frame::StackTargets { cast }
            | Frame::Mana { cast }
            | Frame::SacrificeCost { cast }
            | Frame::SpendMana { cast }
            | Frame::Payment { cast } => {
                cast.stack_id == entry.id && cast.origin == AnnouncementOrigin::Activated
            }
            _ => matches!(
                self.state.rules.frame,
                Frame::Fault { .. } | Frame::Finished { .. }
            ),
        };
        let player_target_domain = artifact_loss_activation(activated)
            || program
                .targets
                .first()
                .is_some_and(|t| t.domain == TargetDomain::PlayerOrPlaneswalker);
        if source_handles.len() != 4
            || (!entry.targets.is_empty()
                && program.targets.first().is_none_or(|t| {
                    !matches!(
                        t.domain,
                        TargetDomain::ControlledBattlefieldCreature
                            | TargetDomain::BattlefieldCreature
                            | TargetDomain::BattlefieldPermanent
                    )
                }))
            || (!entry.player_targets.is_empty()
                && (!player_target_domain
                    || !matches!(entry.player_targets.as_slice(), [p] if p.0 < 4)))
            || (entry.paid.is_some() && player_target_domain && entry.player_targets.len() != 1)
            || (!entry.stack_targets.is_empty()
                && program.targets.first().is_none_or(|t| {
                    !matches!(
                        t.domain,
                        TargetDomain::StackSpell | TargetDomain::StackActivatedOrTriggeredAbility
                    )
                }))
            || entry.total_cost != expected
            || entry.cast_from
                != match activated.functioning_zone {
                    FunctioningZone::Hand => Zone::Hand {
                        owner: entry.controller,
                    },
                    FunctioningZone::Battlefield => Zone::Battlefield,
                    FunctioningZone::Stack | FunctioningZone::Any => {
                        return Err("invalid activation functioning zone".into());
                    }
                }
            || match &entry.paid {
                Some(paid) => *paid != expected,
                None => {
                    !announcing
                        || !(matches!(
                            activated.cost,
                            ActivationCost::Mana { .. }
                                | ActivationCost::DiscardSource { .. }
                                | ActivationCost::Composite { .. }
                                | ActivationCost::SacrificePermanent { .. }
                                | ActivationCost::SacrificeSource
                        ) || (!program.targets.is_empty()))
                }
            }
        {
            return Err("snapshot activated invocation invariant".into());
        }
        if artifact_loss_activation(activated) && entry.paid.is_some() {
            let paid_source = entry.source_facts.as_ref().is_some_and(|source| {
                source.object == entry.source
                    && source.controller == entry.controller
                    && source.zone == Zone::Battlefield
                    && source.tapped
            });
            if !paid_source || self.current(entry.source).is_ok() || entry.player_targets.len() != 1
            {
                return Err("snapshot artifact loss payment/source/target invariant".into());
            }
        }
        if activated.functioning_zone == FunctioningZone::Hand {
            let fact = entry
                .source_facts
                .as_ref()
                .ok_or("missing hand activation source facts")?;
            if !hand_discard_draw_activation(activated)
                || fact.object != entry.source
                || fact.owner != entry.controller
                || fact.zone != entry.cast_from
                || (entry.paid.is_none()
                    && !self.hand_activation_source_payable(entry.controller, entry.source))
                || (entry.paid.is_some() && self.current(entry.source).is_ok())
            {
                return Err("snapshot hand activation cost/source invariant".into());
            }
        }
        Ok(())
    }
    pub(super) fn validate_stack_bindings(&self, entry: &StackInstance) -> Result<(), String> {
        self.validate_cost_lock(entry)?;
        self.validate_damage_bindings(entry)?;
        self.validate_additional_cost_entry(entry)?;
        self.validate_hand_cast_child(entry)?;
        let paid_exile = entry.cast_from == Zone::Exile
            && self.paid_exile_cast_authorized(entry.controller, entry.source);
        if entry.alternative_cost == Some(cascade::AlternativeCost::Bestow)
            && (!entry.kind.is_spell()
                || mtg_contracts::bestow_alternative(self.definition(entry.source.card)?).is_none()
                || (!self.is_spell_copy(entry)
                    && (entry.hand_cast_permission.is_some()
                        || (!paid_exile
                            && !matches!(entry.cast_from,Zone::Hand{owner}|Zone::Command{owner} if owner==entry.controller)))))
        {
            return Err("invalid bestow casting authority".into());
        }
        if entry.alternative_cost == Some(cascade::AlternativeCost::WithoutManaCost)
            && (!entry.kind.is_spell()
                || (!self.is_spell_copy(entry)
                    && entry.cast_from != Zone::Exile
                    && entry.hand_cast_permission.is_none())
                || entry.x.as_ref().is_some_and(|x| *x != Quantity::zero()))
            || (entry.kind.is_spell()
                && entry.cast_from == Zone::Exile
                && !paid_exile
                && entry.alternative_cost != Some(cascade::AlternativeCost::WithoutManaCost))
        {
            return Err("snapshot alternative casting cost/origin invariant".into());
        }
        let x_spell = self.copiable_definition(entry.source)?.x_mana_cost && entry.kind.is_spell();
        let choosing_mode =
            matches!(&self.state.rules.frame, Frame::Modes { cast } if cast.stack_id == entry.id);
        let announcing_x = matches!(&self.state.rules.frame, Frame::AnnounceX { cast } if cast.stack_id == entry.id);
        // An alternative cast fixes X at zero even while a mode is pending.
        let x_fixed_by_alternative = entry.alternative_cost.is_some();
        let x_bound = x_spell && (x_fixed_by_alternative || (!choosing_mode && !announcing_x));
        if entry.x.as_ref().is_some_and(|q| q.is_negative())
            || entry.x.is_some() != x_bound
            || (announcing_x && (!x_spell || x_fixed_by_alternative))
            || entry.death_controller.is_some_and(|p| p.0 >= 4)
            || entry.death_controller.is_some()
                != matches!(&entry.kind,StackKind::Triggered {ability,..} if self.effective_trigger(entry.source, *ability)?.condition==TriggerCondition::OpponentCreatureDies)
        {
            return Err("snapshot X/death LKI binding invariant".into());
        }
        let program = self.program_for(entry)?;
        let mixed = self.mixed_mode_children(entry)?.is_some();
        if mixed && !self.valid_mixed_target_receipt(entry, entry.paid.is_some())? {
            return Err("malformed selected-mode target receipt".into());
        }
        let stack_domain = program.targets.first().is_some_and(|target| {
            matches!(
                target.domain,
                TargetDomain::StackSpell
                    | TargetDomain::ControlledCreatureSpell
                    | TargetDomain::StackActivatedOrTriggeredAbility
            )
        });
        if !entry.stack_targets.is_empty() {
            if (!mixed && (!stack_domain || !entry.targets.is_empty()))
                || !entry.player_targets.is_empty()
                || entry.stack_targets.len() > program.targets.len()
                || entry.announced_stack_targets.len() != 4
                || entry
                    .announced_stack_targets
                    .iter()
                    .any(|entries| entries.len() != entry.stack_targets.len())
                || entry.stack_targets.contains(&entry.id)
            {
                return Err("snapshot stack target binding invariant".into());
            }
        } else if !entry.announced_stack_targets.is_empty() {
            return Err("snapshot stack target announcement invariant".into());
        }
        if !mixed && stack_domain && (!entry.targets.is_empty() || !entry.player_targets.is_empty())
        {
            return Err("snapshot wrong target binding domain".into());
        }
        Ok(())
    }
    pub(super) fn eligible_stack_targets(
        &self,
        domain: TargetDomain,
        exclude: Option<StackId>,
    ) -> Vec<StackId> {
        self.state
            .rules
            .stack
            .iter()
            .filter(|entry| {
                Some(entry.id) != exclude
                    && self.stack_entry_complete(entry)
                    && match domain {
                        TargetDomain::StackSpell => entry.kind.is_spell(),
                        TargetDomain::StackActivatedOrTriggeredAbility => !entry.kind.is_spell(),
                        _ => false,
                    }
            })
            .map(|entry| entry.id)
            .collect()
    }
    pub(super) fn stack_target_legal(
        &self,
        target: StackId,
        countering: StackId,
        domain: TargetDomain,
    ) -> bool {
        self.eligible_stack_targets(domain, Some(countering))
            .contains(&target)
    }
    // CR 101.2/101.3, 113.6 and 608.2b: protection applies when countering
    // the source spell. Its stack target legality is evaluated separately.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    pub(super) fn spell_cannot_be_countered(&self, entry: &StackInstance) -> Result<bool, String> {
        Ok(entry.kind.is_spell()
            && self
                .copiable_definition(entry.source)?
                .static_abilities
                .iter()
                .any(|ability| {
                    ability.functioning_zone == FunctioningZone::Stack
                        && ability.effect == StaticEffect::CantBeCountered
                }))
    }
    pub(super) fn validate_stack_targets(
        &self,
        player: PlayerId,
        entries: &[StackId],
    ) -> Result<(), String> {
        let top = self.top()?;
        let program = self.program()?;
        if top.controller != player
            || entries.len() != program.targets.len()
            || entries.iter().enumerate().any(|(slot, target)| {
                !self
                    .eligible_stack_targets_for(&program.targets[slot], player, Some(top.id))
                    .contains(target)
            })
        {
            return Err("invalid stack targets".into());
        }
        Ok(())
    }
    pub(super) fn bind_stack_targets(&mut self, cast: Casting) -> Result<(), String> {
        let input = self
            .rules_mut()
            .pending
            .take()
            .ok_or("missing stack target input")?;
        let Response::StackTargets { entries } = input.response else {
            return Err("stack target response invariant".into());
        };
        self.validate_stack_targets(input.player, &entries)?;
        let announced = (0..4)
            .map(|viewer| {
                entries
                    .iter()
                    .map(|target| self.stack_handle_id(PlayerId(viewer), *target).cloned())
                    .collect::<Result<Vec<_>, _>>()
            })
            .collect::<Result<Vec<_>, _>>()?;
        let top = self
            .rules_mut()
            .stack
            .last_mut()
            .ok_or("missing stack target invocation")?;
        top.stack_targets = entries;
        top.announced_stack_targets = announced;
        self.rules_mut().frame = self.continue_spell_targeting(cast)?;
        Ok(())
    }
    // CR 701.6a-b/608.2b: remove precisely the bound stack object. Paid costs
    // and physical Commander history are never refunded by countering it.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    pub(super) fn resolve_counter_instruction(
        &mut self,
        frame: Resolving,
        target_slot: u32,
        source_block: u32,
        mill_count: Option<Quantity>,
    ) -> Result<(), String> {
        let slot = target_slot as usize;
        let target = *self
            .bound_stack_target(self.entry(frame.stack_id)?, slot)
            .ok_or("missing counter target binding")?;
        if frame.legal_targets.get(slot) != Some(&true) {
            return self
                .finish_instruction(frame, InstructionOutcome::StackTargetIllegal { target });
        }
        let Some(entry) = self
            .state
            .rules
            .stack
            .iter()
            .find(|entry| entry.id == target)
        else {
            return self.finish_instruction(
                frame,
                InstructionOutcome::StackObjectNoLongerPresent { target },
            );
        };
        // CR 608.2h: the target may leave the stack during the counter event.
        let controller = entry.controller;
        let domain = self
            .program_for(self.entry(frame.stack_id)?)?
            .targets
            .get(slot)
            .ok_or("missing counter target domain")?
            .domain;
        if mill_count.is_some() && domain != TargetDomain::StackSpell {
            return Err("counter and mill requires a spell target".into());
        }
        let source = entry.source;
        let proposed = ProposedEvent::CounterStackObject {
            target,
            countering: frame.stack_id,
            domain,
        };
        if entry.kind.is_spell()
            && !self.spell_cannot_be_countered(entry)?
            && self.has_self_graveyard_replacement(source)?
        {
            let owner = self.state.rules.cards[&source.card].owner;
            return self.begin_graveyard_effect(
                frame,
                proposed,
                source_block,
                owner,
                vec![source],
                graveyard_replacement::GraveyardCompletion::Counter {
                    target,
                    source,
                    controller,
                    mill: mill_count,
                },
            );
        }
        let actual = self.commit(proposed, self.instruction_cause(&frame, source_block)?)?;
        self.complete_counter_outcome(
            frame,
            source_block,
            target,
            controller,
            mill_count,
            Some(actual),
        )
    }

    pub(super) fn complete_counter_outcome(
        &mut self,
        frame: Resolving,
        source_block: u32,
        target: StackId,
        controller: PlayerId,
        mill_count: Option<Quantity>,
        actual: Option<EventOutcome>,
    ) -> Result<(), String> {
        let outcome = match actual {
            Some(EventOutcome::StackObjectCountered { movement, .. }) => {
                InstructionOutcome::Countered { target, movement }
            }
            Some(EventOutcome::CounterPrevented { .. }) => {
                InstructionOutcome::CounterPrevented { target }
            }
            None => InstructionOutcome::StackObjectNoLongerPresent { target },
            _ => return Err("missing counter outcome".into()),
        };
        if let Some(count) = mill_count {
            if self.is_live(controller) && count > Quantity::zero() {
                let cards = self.library_prefix(controller, &count)?;
                return self.complete_library_effect_with(
                    frame,
                    ProposedEvent::Mill {
                        player: controller,
                        count,
                        cards,
                    },
                    source_block,
                    graveyard_replacement::GraveyardCompletion::MillAfterCounter {
                        outcome: Box::new(outcome),
                    },
                );
            }
            self.commit_mill(
                controller,
                count,
                self.instruction_cause(&frame, source_block)?,
            )?;
        }
        self.finish_instruction(frame, outcome)
    }
}
