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

/// CR 602.5b-c, 400.7: a turn limit follows the ability slot of this
/// incarnation through control changes. A zone change creates a new object;
/// a transformed permanent's other face has its own abilities (CR 712.18).
/// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActivationUse {
    pub source: ObjectId,
    #[serde(default, skip_serializing_if = "CardFace::is_front")]
    pub face: CardFace,
    pub ability: u32,
}

impl RulesTxn {
    pub(super) fn activation_restriction_allows(
        &self,
        player: PlayerId,
        source: ObjectId,
        ability: u32,
        restriction: ActivationRestriction,
    ) -> bool {
        let own_turn = self.state.rules.active == Some(player);
        let step = self.state.rules.turn.step;
        let in_window = match restriction.window {
            None => true,
            Some(ActivationWindow::YourTurn) => own_turn,
            Some(ActivationWindow::YourUpkeep) => own_turn && step == TurnStep::Upkeep,
            Some(ActivationWindow::YourTurnBeforeAttackers) => {
                own_turn
                    && matches!(
                        step,
                        TurnStep::Upkeep
                            | TurnStep::Draw
                            | TurnStep::PrecombatMain
                            | TurnStep::BeginningCombat
                    )
            }
        };
        in_window
            && (!restriction.once_each_turn
                || !self.state.rules.activation_uses.contains(&ActivationUse {
                    source,
                    face: self.current(source).map_or(source.face, Object::face),
                    ability,
                }))
    }

    pub(super) fn validate_activation_uses(&self) -> Result<(), String> {
        for key in &self.state.rules.activation_uses {
            if self.current(key.source)?.zone != Zone::Battlefield
                || !self.activation_is_limited(key.source, key.face, key.ability)?
            {
                return Err("snapshot activation use invariant".into());
            }
        }
        Ok(())
    }

    fn activation_is_limited(
        &self,
        source: ObjectId,
        face: CardFace,
        ability: u32,
    ) -> Result<bool, String> {
        Ok(self
            .face_definition(source.card, face)?
            .activated
            .get(ability as usize)
            .ok_or("missing limited activation slot")?
            .restriction
            .is_some_and(|r| r.once_each_turn))
    }

    /// CR 601.2i, 602.2b, 602.5b: only a successfully completed activation
    /// uses the limit. A reversed announcement never used that ability.
    pub(super) fn record_ability_activated(
        &mut self,
        stack_id: StackId,
        source: ObjectId,
        player: PlayerId,
        ability: u32,
    ) -> Result<(), String> {
        // The activated face, fixed as the ability was put on the stack.
        let face = self.entry(stack_id)?.source_face;
        if self.activation_is_limited(source, face, ability)?
            && self.current(source).is_ok()
            && !self.rules_mut().activation_uses.insert(ActivationUse {
                source,
                face,
                ability,
            })
        {
            return Err("activation limit already used".into());
        }
        self.record_event(Event::AbilityActivated {
            stack_id,
            source,
            player,
            ability,
        })
    }

    /// CR 602.2a, 113.6j: a graveyard card's activated ability is
    /// activated by that card's owner, from that owner's own graveyard.
    pub(super) fn graveyard_activation_source_payable(
        &self,
        player: PlayerId,
        source: ObjectId,
    ) -> bool {
        self.is_live(player)
            && self
                .current(source)
                .is_ok_and(|o| o.zone == (Zone::Graveyard { owner: player }))
            && self
                .state
                .rules
                .cards
                .get(&source.card)
                .is_some_and(|c| c.owner == player)
    }

    /// The off-battlefield source still sits where its cost moves it from.
    pub(super) fn zone_source_payable(
        &self,
        zone: FunctioningZone,
        player: PlayerId,
        source: ObjectId,
    ) -> bool {
        match zone {
            FunctioningZone::Hand => self.hand_activation_source_payable(player, source),
            FunctioningZone::Graveyard => self.graveyard_activation_source_payable(player, source),
            FunctioningZone::Battlefield | FunctioningZone::Stack | FunctioningZone::Any => false,
        }
    }

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
    /// CR 602.2b, 601.2c: the announcement stage for an activation's first
    /// target domain.
    pub(super) fn activation_target_frame(domain: TargetDomain, cast: Casting) -> Frame {
        match domain {
            TargetDomain::AnyPlayer => Frame::PlayerTargets { cast },
            TargetDomain::StackSpell | TargetDomain::StackActivatedOrTriggeredAbility => {
                Frame::StackTargets { cast }
            }
            TargetDomain::ControlledBattlefieldCreature
            | TargetDomain::ControlledBattlefieldPermanent
            | TargetDomain::BattlefieldCreature
            | TargetDomain::BattlefieldPermanent
            | TargetDomain::OwnGraveyardCard
            | TargetDomain::OwnGraveyardCreatureCard => Frame::Targets { cast },
            _ => Frame::EntityTargets { cast },
        }
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
            ActivationCost::ChooseObjects(cost) => cost.mana.clone().unwrap_or_else(zero_mana_cost),
            ActivationCost::Mana { cost } => cost.clone(),
            ActivationCost::DiscardSource { mana } | ActivationCost::ExileSource { mana } => {
                mana.clone()
            }
            ActivationCost::Composite { costs } => {
                costs.mana.clone().unwrap_or_else(zero_mana_cost)
            }
            _ => ManaCost {
                generic: Quantity::zero(),
                colored: vec![],
            },
        };
        let chosen = matches!(
            cost,
            ActivationCost::SacrificePermanent { .. } | ActivationCost::ChooseObjects(_)
        );
        let mana = matches!(
            cost,
            ActivationCost::Mana { .. }
                | ActivationCost::DiscardSource { .. }
                | ActivationCost::ExileSource { .. }
                | ActivationCost::SacrificePermanent { mana: Some(_), .. }
        ) || matches!(&cost, ActivationCost::Composite { costs } if costs.mana.is_some())
            || matches!(&cost, ActivationCost::ChooseObjects(cost) if cost.mana.is_some());
        let activated = &self.copiable_definition(object)?.activated[ability as usize];
        // CR 602.2b, 700.2a: modes are chosen first, then their targets.
        let modal = activated.modal.is_some();
        let targeted = !activated.program.targets.is_empty() || modal;
        let entry = StackInstance {
            mode_mask: None,
            hybrid_colors: vec![],
            cost_lock: None,
            selected_additional_costs: None,

            x: None,
            death_controller: None,
            sacrificed: None,
            sacrificed_set: vec![],
            chosen_payment: vec![],
            effect_source_handles: vec![],
            source_facts: { Some(self.object_before(object)?) },
            entity_targets: vec![],
            announced_entity_targets: vec![],
            kind: StackKind::Activated {
                ability,
                source_handles,
            },
            // CR 602.2a, 113.7a: the activated face, fixed as it is put on
            // the stack (CR 701.27f).
            source_face: self.current(object)?.face(),
            source_transforms: self.current(object)?.transforms,
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
        if targeted {
            if cost != ActivationCost::TapSource
                && !matches!(
                    cost,
                    ActivationCost::Mana { .. }
                        | ActivationCost::ExileSource { .. }
                        | ActivationCost::Composite { .. }
                        | ActivationCost::SacrificePermanent { .. }
                        | ActivationCost::ChooseObjects(_)
                        | ActivationCost::SacrificeSource
                )
            {
                return Err("unsupported targeted activation cost".into());
            }
            let cast = Casting {
                sacrifices: vec![],
                origin: AnnouncementOrigin::Activated,
                spending: None,
                spending_restricted: vec![],
                stack_id: id,
                original_object: object,
                prior_passes: self.state.rules.passes.clone(),
                mana_activations: vec![],
                undo_mana: false,
                revealed_hand_consumed: false,
            };
            self.rules_mut().frame = if modal {
                Frame::ChooseModes { cast }
            } else {
                Self::activation_target_frame(
                    self.copiable_definition(object)?.activated[ability as usize]
                        .program
                        .targets[0]
                        .domain,
                    cast,
                )
            };
        } else if mana {
            self.rules_mut().frame = Frame::Mana {
                cast: Casting {
                    sacrifices: vec![],
                    origin: AnnouncementOrigin::Activated,
                    spending: None,
                    spending_restricted: vec![],
                    stack_id: id,
                    original_object: object,
                    prior_passes: self.state.rules.passes.clone(),
                    mana_activations: vec![],
                    undo_mana: false,
                    revealed_hand_consumed: false,
                },
            };
        } else if chosen {
            // CR 602.2b/601.2g-h: no mana opportunity for a cost with no mana.
            let cast = Casting {
                sacrifices: vec![],
                origin: AnnouncementOrigin::Activated,
                spending: Some(BTreeMap::new()),
                spending_restricted: vec![],
                stack_id: id,
                original_object: object,
                prior_passes: self.state.rules.passes.clone(),
                mana_activations: vec![],
                undo_mana: false,
                revealed_hand_consumed: false,
            };
            self.rules_mut().frame = if matches!(cost, ActivationCost::ChooseObjects(_)) {
                Self::chosen_cost_frame(cast)
            } else {
                Frame::SacrificeCost { cast }
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
                    | ActivationCost::ExileSource { .. }
                    | ActivationCost::ChooseObjects(_)
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
            self.record_ability_activated(id, object, player, ability)?;
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
        source_handles: &[publish::StoredObjectHandle],
    ) -> Result<(), String> {
        let definition = self.stack_definition(entry)?;
        let activated = definition
            .activated
            .get(ability as usize)
            .ok_or("snapshot missing activated definition")?;
        let expected = match &activated.cost {
            ActivationCost::SacrificePermanent { mana, .. } => {
                mana.clone().unwrap_or_else(zero_mana_cost)
            }
            ActivationCost::ChooseObjects(cost) => cost.mana.clone().unwrap_or_else(zero_mana_cost),
            ActivationCost::Mana { cost } => cost.clone(),
            ActivationCost::DiscardSource { mana } | ActivationCost::ExileSource { mana } => {
                mana.clone()
            }
            ActivationCost::Composite { costs } => {
                costs.mana.clone().unwrap_or_else(zero_mana_cost)
            }
            _ => ManaCost {
                generic: Quantity::zero(),
                colored: vec![],
            },
        };
        let program = self.program_for(entry)?;
        if activated.restriction.is_some_and(|r| r.once_each_turn)
            && self.current(entry.source).is_ok()
            && (self.state.rules.activation_uses.contains(&ActivationUse {
                source: entry.source,
                face: entry.source_face,
                ability,
            }) != entry.paid.is_some())
        {
            return Err("snapshot activation payment/use invariant".into());
        }
        let stage = self.underlying_mana_frame(&self.state.rules.frame)?;
        let announcing = match stage {
            Frame::ChooseModes { cast }
            | Frame::Targets { cast }
            | Frame::PlayerTargets { cast }
            | Frame::EntityTargets { cast }
            | Frame::StackTargets { cast }
            | Frame::Mana { cast }
            | Frame::SacrificeCost { cast }
            | Frame::ChosenObjectCost { cast, .. }
            | Frame::SpendMana { cast }
            | Frame::Payment { cast } => {
                cast.stack_id == entry.id && cast.origin == AnnouncementOrigin::Activated
            }
            _ => matches!(
                self.state.rules.frame,
                Frame::Fault { .. } | Frame::Finished { .. }
            ),
        };
        let player_target_domain = program
            .targets
            .first()
            .is_some_and(|t| t.domain == TargetDomain::PlayerOrPlaneswalker);
        if source_handles.len() != 4
            || (!entry.targets.is_empty()
                && program.targets.first().is_none_or(|t| {
                    !matches!(
                        t.domain,
                        TargetDomain::ControlledBattlefieldCreature
                            | TargetDomain::ControlledBattlefieldPermanent
                            | TargetDomain::BattlefieldCreature
                            | TargetDomain::BattlefieldPermanent
                            | TargetDomain::OwnGraveyardCard
                            | TargetDomain::OwnGraveyardCreatureCard
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
                    FunctioningZone::Graveyard => Zone::Graveyard {
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
                                | ActivationCost::ExileSource { .. }
                                | ActivationCost::Composite { .. }
                                | ActivationCost::SacrificePermanent { .. }
                                | ActivationCost::ChooseObjects(_)
                                | ActivationCost::SacrificeSource
                        ) || (!program.targets.is_empty()))
                }
            }
        {
            return Err("snapshot activated invocation invariant".into());
        }
        if let ActivationCost::ChooseObjects(cost) = &activated.cost
            && entry.paid.is_none()
        {
            self.validate_chosen_announcement_stage(entry, cost, stage)?;
        }

        if matches!(
            activated.functioning_zone,
            FunctioningZone::Hand | FunctioningZone::Graveyard
        ) {
            let fact = entry
                .source_facts
                .as_ref()
                .ok_or("missing hand activation source facts")?;
            // CR 602.2b, 601.2h: the source card is the cost, so an unpaid
            // entry keeps it in place and a paid entry has moved it away.
            let shape = match activated.cost {
                ActivationCost::ExileSource { .. } => {
                    activated.functioning_zone == FunctioningZone::Graveyard
                }
                ActivationCost::DiscardSource { .. } => hand_discard_activation(activated),
                _ => false,
            };
            if !shape
                || fact.object != entry.source
                || fact.owner != entry.controller
                || fact.zone != entry.cast_from
                || (entry.paid.is_none()
                    && !self.zone_source_payable(
                        activated.functioning_zone,
                        entry.controller,
                        entry.source,
                    ))
                || (entry.paid.is_some() && self.current(entry.source).is_ok())
            {
                return Err("snapshot hand activation cost/source invariant".into());
            }
        }
        Ok(())
    }
    pub(super) fn validate_stack_bindings(&self, entry: &StackInstance) -> Result<(), String> {
        self.validate_paid_chosen_cost(entry)?;
        // CR 601.2a, 702.34a, 903.8: a spell is cast by its owner from that
        // owner's own hand, from the command zone as that owner's commander,
        // or from that owner's own graveyard only with flashback. No other
        // origin exists for a cast spell (a copy is never cast).
        if entry.kind.is_spell() && !self.is_spell_copy(entry) {
            let owner = self
                .state
                .rules
                .cards
                .get(&entry.source.card)
                .ok_or("missing spell physical card")?
                .owner;
            let consistent = entry.controller == owner
                && match entry.cast_from {
                    Zone::Hand { owner: from } => from == owner,
                    Zone::Command { owner: from } => {
                        from == owner
                            && self
                                .state
                                .rules
                                .commanders
                                .history(entry.source.card)
                                .is_some()
                    }
                    Zone::Graveyard { owner: from } => {
                        from == owner && self.stack_definition(entry)?.flashback.is_some()
                    }
                    // CR 715.3d: an adventurer exiled by its resolving
                    // Adventure, cast normally. Replay authenticates the
                    // exiled incarnation's permission.
                    Zone::Exile => self.adventurer_cast_from_exile(entry)?,
                    Zone::Battlefield | Zone::Library { .. } | Zone::Stack => false,
                };
            if !consistent {
                return Err("snapshot spell casting origin invariant".into());
            }
        }
        self.validate_modes(entry)?;
        self.validate_cost_lock(entry)?;
        self.validate_damage_bindings(entry)?;
        self.validate_additional_cost_entry(entry)?;

        if entry.kind.is_spell()
            && entry.cast_from == Zone::Exile
            && !self.adventurer_cast_from_exile(entry)?
        {
            return Err("snapshot alternative casting cost/origin invariant".into());
        }
        let x_spell = self.stack_definition(entry)?.x_mana_cost && entry.kind.is_spell();
        let announcing_x = matches!(&self.state.rules.frame, Frame::AnnounceX { cast } if cast.stack_id == entry.id);
        // CR 601.2b: optional additional costs and hybrid symbols are
        // announced before X, so X stays unbound through those stages.
        let before_x = announcing_x
            || matches!(&self.state.rules.frame,
                Frame::ChooseModes { cast } | Frame::AdditionalCosts { cast } | Frame::HybridCost { cast } if cast.stack_id == entry.id);
        let x_bound = x_spell && !before_x;
        // A game can end (CR 104.2a) while a spell is still being announced;
        // its unpaid entry then keeps whatever it had announced.
        let unfinished = entry.paid.is_none()
            && matches!(
                self.state.rules.frame,
                Frame::Fault { .. } | Frame::Finished { .. }
            );
        if entry.x.as_ref().is_some_and(|q| q.is_negative())
            || (entry.x.is_some() != x_bound && !(unfinished && x_spell))
            || (announcing_x && !x_spell)
            || entry.death_controller.is_some_and(|p| p.0 >= 4)
            || entry.death_controller.is_some()
                != matches!(&entry.kind,StackKind::Triggered {ability: ability @ EffectiveTriggerAbility::Intrinsic { .. },..} if triggers::binds_death_controller(&*self.effective_trigger(entry.source, entry.source_face, *ability)?))
        {
            return Err("snapshot X/death LKI binding invariant".into());
        }
        let program = self.program_for(entry)?;
        let stack_domain = program.targets.first().is_some_and(|target| {
            matches!(
                target.domain,
                TargetDomain::StackSpell
                    | TargetDomain::ControlledCreatureSpell
                    | TargetDomain::StackActivatedOrTriggeredAbility
            )
        });
        if !entry.stack_targets.is_empty() {
            if !stack_domain
                || !entry.targets.is_empty()
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
        if stack_domain && (!entry.targets.is_empty() || !entry.player_targets.is_empty()) {
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
        if !entry.kind.is_spell() {
            return Ok(false);
        }
        self.has_uncounterable_static(entry.source, entry.controller)
    }

    // CR 701.6a-b/608.2b: remove precisely the bound stack object. Paid costs
    // and physical Commander history are never refunded by countering it.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
}
