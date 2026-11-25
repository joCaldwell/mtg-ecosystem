use super::*;

/// Owned, preflighted proposal. Only a permission authority in this module can
/// construct one. It is consumed before any announcement decision is exposed;
/// ongoing announcement ownership remains in `Casting` (including nested mana).
/// Priority and the exact one-use cascade permission supply separate authorities;
/// both consume this proposal and use the same announcement/payment stages.
struct SpellAnnouncement {
    player: PlayerId,
    original_object: ObjectId,
    cast_from: Zone,
}

impl RulesTxn {
    pub(super) fn announcement_mana_frame(&mut self, cast: Casting) -> Result<Frame, String> {
        self.lock_spell_cost()?;
        Ok(Frame::Mana { cast })
    }

    fn priority_spell_announcement(
        &self,
        player: PlayerId,
        card: CardId,
    ) -> Result<SpellAnnouncement, String> {
        if !matches!(self.state.rules.frame, Frame::Priority) || self.state.rules.priority != player
        {
            return Err("spell proposal requires its ordinary priority owner".into());
        }
        self.validate_cast_permission(player)?;
        let object = self.state.rules.objects.get(&card).ok_or("invalid cast")?;
        let definition = self.definition(card)?;
        // CR 702.34a: flashback permits casting from its owner's graveyard.
        if !(object.zone == (Zone::Hand { owner: player })
            || (object.zone == (Zone::Command { owner: player })
                && self.state.rules.commanders.history(card).is_some())
            || (object.zone == (Zone::Graveyard { owner: player })
                && definition.flashback.is_some()))
            || self.state.rules.cards[&card].owner != player
            || definition.spell.is_none()
        {
            return Err("invalid or unsupported cast".into());
        }
        let program = definition.spell.as_ref().ok_or("missing cast program")?;
        // CR 601.2/702.8a: only the permission authority supplies timing/zone
        // access. Global prohibitions still apply to every eventual authority.
        // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
        if program.timing == SpellTiming::MainPhase
            && !self.main_phase_action(player)
            && !self.has_flash_permission(object, player)?
        {
            return Err("main-phase spell casting requires active-player main-phase priority and empty stack".into());
        }
        if let Some(reason) = self.spell_proposal_obstacle(program, player, &definition.characteristics)
        {
            return Err(reason.into());
        }
        if let Some(modal) = &definition.modal_spell
            && !self.modal_has_selection(modal, player, &definition.characteristics)
        {
            return Err("no legal mode selection for this proposal".into());
        }
        Ok(SpellAnnouncement {
            player,
            original_object: object.id,
            cast_from: object.zone,
        })
    }

    // Priority and resolution permissions differ on timing and origin. Once
    // those are established, target and mandatory sacrifice availability are
    // properties of the same proposed spell.
    pub(super) fn spell_proposal_obstacle(
        &self,
        program: &SpellProgram,
        player: PlayerId,
        source: &Characteristics,
    ) -> Option<&'static str> {
        if program
            .additional_sacrifice
            .as_ref()
            .is_some_and(|filter| self.sacrifice_candidates(player, filter).is_empty())
        {
            return Some("no payable sacrifice cost");
        }
        if !self.has_target_domain(program, player, source) {
            return Some("no legal target for this proposal");
        }
        None
    }

    pub(super) fn validate_priority_spell_cast(
        &self,
        player: PlayerId,
        card: CardId,
    ) -> Result<(), String> {
        self.priority_spell_announcement(player, card).map(|_| ())
    }

    pub(crate) fn announce_priority_spell(
        &mut self,
        player: PlayerId,
        card: CardId,
    ) -> Result<(), String> {
        let announcement = self.priority_spell_announcement(player, card)?;
        self.begin_spell_announcement(announcement)
    }

    // CR 601.2a-i: one proposal constructor owns the move, fresh stack identity
    // and unfinished payment marker. No cast occurrence is emitted here.
    fn begin_spell_announcement(&mut self, announcement: SpellAnnouncement) -> Result<(), String> {
        let SpellAnnouncement {
            player,
            original_object,
            cast_from,
        } = announcement;
        let card = original_object.card;
        self.check_stack_capacity(1)?;
        let total_cost = self.casting_cost(card, player, cast_from)?;
        let id = StackId(self.state.rules.next_stack_id);
        self.rules_mut().next_stack_id = self
            .state
            .rules
            .next_stack_id
            .checked_add(1)
            .ok_or("stack ID resource limit")?;
        self.record_event(Event::SpellProposed { card, stack_id: id })?;
        let known_before = self.known_hand_count(original_object.card);
        let source = self
            .move_object_for(
                original_object,
                Zone::Stack,
                BatchCause::Casting {
                    stack_id: id,
                    player,
                    source: original_object,
                },
            )?
            .successor;
        let revealed_hand_consumed = self.known_hand_count(original_object.card) < known_before;
        let effect_source_handles = self.capture_trigger_source_handles(source)?;
        let source_facts = self.object_before(source)?;
        let entry = StackInstance {
            mode_mask: None,
            hybrid_colors: vec![],
            cost_lock: None,
            selected_additional_costs: None,
            x: None,
            death_controller: None,
            sacrificed: None,
            sacrificed_set: vec![],
            effect_source_handles,
            source_facts: Some(source_facts),
            entity_targets: vec![],
            announced_entity_targets: vec![],
            stack_targets: vec![],
            announced_stack_targets: vec![],
            kind: StackKind::Spell,
            player_targets: vec![],
            id,
            source,
            controller: player,
            targets: vec![],
            announced_targets: vec![vec![]; 4],
            paid: None,
            total_cost,
            cast_from,
        };
        self.rules_mut().stack.push(entry);
        let cast = Casting {
            sacrifices: vec![],
            origin: AnnouncementOrigin::Spell,
            spending: None,
            spending_restricted: vec![],
            stack_id: id,
            original_object,
            prior_passes: self.state.rules.passes.clone(),
            mana_activations: vec![],
            undo_mana: false,
            revealed_hand_consumed,
        };
        self.rules_mut().frame = self.spell_announcement_frame(cast)?;
        self.rules_mut().passes.clear();
        Ok(())
    }

    // All supported spells share modes/X/target routing. This selects a stage,
    // never grants a casting permission or changes a target filter's meaning.
    pub(super) fn spell_announcement_frame(&mut self, cast: Casting) -> Result<Frame, String> {
        let stack = self.top()?;
        if stack.id != cast.stack_id || !stack.kind.is_spell() || !cast.origin.is_spell() {
            return Err("spell announcement binding invariant".into());
        }
        let definition = self.copiable_definition(stack.source)?;
        if definition.modal_spell.is_some() && stack.mode_mask.is_none() {
            return Ok(Frame::ChooseModes { cast });
        }
        if !definition
            .spell
            .as_ref()
            .ok_or("missing spell")?
            .optional_additional_costs
            .is_empty()
            && stack.selected_additional_costs.is_none()
        {
            return Ok(Frame::AdditionalCosts { cast });
        }
        if !definition.characteristics.hybrid_mana.is_empty() && stack.hybrid_colors.is_empty() {
            return Ok(Frame::HybridCost { cast });
        }
        if definition.x_mana_cost && stack.x.is_none() {
            return Ok(Frame::AnnounceX { cast });
        }
        let program = self.program_for(stack)?;
        Ok(match program.targets.first().map(|target| target.domain) {
            None => self.announcement_mana_frame(cast)?,
            Some(
                TargetDomain::StackSpell
                | TargetDomain::ControlledCreatureSpell
                | TargetDomain::StackActivatedOrTriggeredAbility,
            ) => Frame::StackTargets { cast },

            Some(TargetDomain::PlayerGraveyardCards { .. }) => {
                return Err("dependent targets need their player".into());
            }
            Some(TargetDomain::AnyDamageTarget) => Frame::EntityTargets { cast },
            Some(TargetDomain::PlayerOrPlaneswalker) => {
                return Err("player-or-planeswalker requires an activated mode".into());
            }
            Some(TargetDomain::OpponentPlayer | TargetDomain::AnyPlayer) => {
                Frame::PlayerTargets { cast }
            }
            Some(
                TargetDomain::ControlledBattlefieldCreature
                | TargetDomain::ControlledBattlefieldPermanent
                | TargetDomain::BattlefieldPermanent
                | TargetDomain::BattlefieldCreature
                | TargetDomain::CreatureOrPlaneswalker
                | TargetDomain::BlockingBattlefieldCreature
                | TargetDomain::OwnGraveyardCreatureCard
                | TargetDomain::OwnGraveyardCard,
            ) => Frame::Targets { cast },
        })
    }
    /// CR 702.34a: a spell whose flashback cost was paid (the one way a spell
    /// is cast from a graveyard) is exiled instead of moving anywhere else as
    /// it leaves the stack. A copy was never cast and keeps `destination`.
    pub(super) fn stack_departure(&self, entry: &StackInstance, destination: Zone) -> Zone {
        if entry.kind.is_spell() && matches!(entry.cast_from, Zone::Graveyard { .. }) {
            Zone::Exile
        } else {
            destination
        }
    }
    /// CR 800.4a: a departed player casts nothing.
    pub(crate) fn validate_cast_permission(&self, player: PlayerId) -> Result<(), String> {
        if !self.is_live(player) {
            return Err("departed player cannot cast spells".into());
        }
        Ok(())
    }
    pub(crate) fn validate_mana_activation(
        &self,
        player: PlayerId,
        object: ObjectId,
    ) -> Result<(), String> {
        if self.is_permanent_mana(object) {
            return self.validate_permanent_mana_activation(player, object);
        }
        if self.reserved_mana_source(object)? {
            return Err("mana source is already announcing".into());
        }
        self.intrinsic_choice_colors(player, object)?;
        Ok(())
    }

    // CR 601.2g / 605.3a-b: intrinsic mana abilities resolve immediately,
    // including in the casting mana window. No player receives priority here.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    pub(super) fn activate_selected_intrinsic_mana(
        &mut self,
        input: &AcceptedInput,
        object: ObjectId,
        color: Color,
    ) -> Result<ManaActivationUndo, String> {
        self.validate_intrinsic_choice(input.player, object, color)?;
        self.commit_unrestricted_tap_mana(input, object, color)
    }

    /// Called only after binding a concrete intrinsic ability.
    fn commit_unrestricted_tap_mana(
        &mut self,
        input: &AcceptedInput,
        object: ObjectId,
        color: Color,
    ) -> Result<ManaActivationUndo, String> {
        let receipt = ManaActivationUndo {
            permanent: None,
            object,
            player: input.player,
            color,
            amount: Quantity::from(1),
            mana_was_absent: !self.state.rules.players[input.player.0 as usize]
                .mana
                .contains_key(&color),
        };
        self.commit_batch(
            BatchCause::ManaActivation {
                decision_id: input.decision_id,
                player: input.player,
                source: object,
            },
            EventGroup::Ordered {
                children: vec![
                    EventGroup::Occurrence {
                        proposed: ProposedEvent::Tap { object },
                    },
                    EventGroup::Occurrence {
                        proposed: ProposedEvent::AddMana {
                            restriction: None,
                            player: input.player,
                            color,
                            amount: receipt.amount.clone(),
                            source: object,
                        },
                    },
                ],
            },
        )?;
        Ok(receipt)
    }

    // CR 733.1-2: undo only this attempted cast. Legal mana abilities may be
    // kept; this bounded profile offers keep-all or undo-all. External player
    // departures and mana floated before the attempt cannot be reversed.
    pub(super) fn reverse_cast(
        &mut self,
        cast: Casting,
        reason: &str,
        undo_mana: bool,
    ) -> Result<(), String> {
        let stack = self.top()?.clone();
        if stack.id != cast.stack_id || !self.is_live(stack.controller) {
            return Err("casting reversal source/player invariant".into());
        }
        // CR 733.1: a kept activation (life paid, riders) keeps the whole set.
        let undo_mana = undo_mana && mana_permanents::reversible_receipts(&cast.mana_activations);
        if undo_mana && !cast.mana_activations.is_empty() {
            let mut children = vec![];
            for receipt in cast.mana_activations.iter().rev() {
                if let Some(permanent) = &receipt.permanent {
                    children.push(EventGroup::Occurrence {
                        proposed: ProposedEvent::ReversePermanentMana {
                            receipt: permanent.clone(),
                        },
                    });
                    continue;
                }
                let source = match self.current(receipt.object) {
                    Ok(o) if o.present_on_battlefield() && o.tapped => Some(o.id),
                    Ok(_) => return Err("casting reversal source changed unexpectedly".into()),
                    Err(_) if !self.is_live(self.state.rules.cards[&receipt.object.card].owner) => {
                        None
                    }
                    Err(_) => return Err("casting reversal lost surviving mana source".into()),
                };
                children.push(EventGroup::Occurrence {
                    proposed: ProposedEvent::ReverseMana {
                        player: receipt.player,
                        color: receipt.color,
                        amount: receipt.amount.clone(),
                        source,
                        remove_zero: receipt.mana_was_absent,
                    },
                });
            }
            self.commit_batch(
                BatchCause::CastReversal {
                    stack_id: cast.stack_id,
                    player: stack.controller,
                },
                EventGroup::Ordered { children },
            )?;
        }
        self.rules_mut().stack.pop();
        if cast.origin.is_spell() {
            let owner = self.state.rules.cards[&cast.original_object.card].owner;
            let o = self
                .rules_mut()
                .objects
                .get_mut(&cast.original_object.card)
                .ok_or("missing reversal object")?;
            o.id = cast.original_object;
            o.zone = stack.cast_from;
            o.controller = owner;
            o.default_controller = owner;
            o.tapped = false;
            // The card returns publicly to the hand it was announced from.
            if cast.revealed_hand_consumed {
                if !matches!(stack.cast_from, Zone::Hand { owner: p } if p == owner) {
                    return Err("known hand reversal outside its owner's hand".into());
                }
                self.restore_known_hand_copy(cast.original_object.card);
            }
        }
        self.rules_mut().pending = None;
        self.rules_mut().priority = stack.controller;
        // CR 117.4: keeping a legal mana activation interrupts the old pass
        // round. Undoing all activations restores the pre-attempt live passes.
        let live = self.live_players();
        self.rules_mut().passes = if undo_mana || cast.mana_activations.is_empty() {
            cast.prior_passes
                .into_iter()
                .filter(|p| live.contains(p))
                .collect()
        } else {
            vec![]
        };
        self.rules_mut().frame = Frame::Priority;
        self.record_event(Event::CastReversed {
            stack_id: cast.stack_id,
            reason: reason.into(),
        })?;
        Ok(())
    }
}
