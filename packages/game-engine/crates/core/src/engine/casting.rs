use super::*;

/// Owned, preflighted proposal. Only a permission authority in this module can
/// construct one. It is consumed before any announcement decision is exposed;
/// ongoing announcement ownership remains in `Casting` (including nested mana).
/// Priority and the exact one-use cascade permission supply separate authorities;
/// both consume this proposal and use the same announcement/payment stages.
struct SpellAnnouncement {
    bestow: bool,
    player: PlayerId,
    original_object: ObjectId,
    cast_from: Zone,
    completion: CastCompletion,
}

impl Game {
    fn priority_spell_announcement(
        &self,
        player: PlayerId,
        card: CardId,
        bestow: bool,
    ) -> Result<SpellAnnouncement, String> {
        if !matches!(self.state.rules.frame, Frame::Priority) || self.state.rules.priority != player
        {
            return Err("spell proposal requires its ordinary priority owner".into());
        }
        self.validate_cast_permission(player)?;
        let object = self.state.rules.objects.get(&card).ok_or("invalid cast")?;
        let definition = self.definition(card)?;
        if !(object.zone == (Zone::Hand { owner: player })
            || (object.zone == (Zone::Command { owner: player })
                && self.state.rules.commanders.history(card).is_some()))
            || self.state.rules.cards[&card].owner != player
            || definition.spell.is_none()
        {
            return Err("invalid or unsupported cast".into());
        }
        let program = if bestow {
            mtg_contracts::bestow_alternative(definition)
                .ok_or("missing bestow permission")?
                .1
        } else {
            definition.spell.as_ref().ok_or("missing cast program")?
        };
        // CR 601.2/702.8a: only the permission authority supplies timing/zone
        // access. Global prohibitions still apply to every eventual authority.
        // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
        if program.timing == SpellTiming::MainPhase && !self.main_phase_action(player) {
            return Err("main-phase spell casting requires active-player main-phase priority and empty stack".into());
        }
        if let Some(reason) = self.spell_proposal_obstacle(
            program,
            player,
            &self.proposed_cast_characteristics(card, bestow)?,
        ) {
            return Err(reason.into());
        }
        Ok(SpellAnnouncement {
            bestow,
            player,
            original_object: object.id,
            cast_from: object.zone,
            completion: CastCompletion::Priority,
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
        self.priority_spell_announcement(player, card, false)
            .map(|_| ())
    }

    pub(super) fn announce_priority_spell(
        &mut self,
        player: PlayerId,
        card: CardId,
    ) -> Result<(), String> {
        let announcement = self.priority_spell_announcement(player, card, false)?;
        self.begin_spell_announcement(announcement)
    }

    pub(super) fn validate_priority_bestow(
        &self,
        player: PlayerId,
        card: CardId,
    ) -> Result<(), String> {
        self.priority_spell_announcement(player, card, true)
            .map(|_| ())
    }
    pub(super) fn announce_priority_bestow(
        &mut self,
        player: PlayerId,
        card: CardId,
    ) -> Result<(), String> {
        let announcement = self.priority_spell_announcement(player, card, true)?;
        self.begin_spell_announcement(announcement)
    }
    pub(super) fn is_bestowed_spell(&self, object: ObjectId) -> bool {
        // CR 702.103e: the Aura effect ends as resolution starts if its target
        // is illegal. The stack entry keeps the alternative payment as history.
        if let Frame::Resolve { frame } = &self.state.rules.frame
            && frame.spell.source == object
            && frame.spell.kind.is_spell()
            && frame.legal_targets == [false]
        {
            return false;
        }
        self.state.rules.stack.iter().any(|s| {
            s.source == object
                && s.kind.is_spell()
                && s.alternative_cost == Some(cascade::AlternativeCost::Bestow)
        })
    }
    pub(super) fn proposed_cast_characteristics(
        &self,
        card: CardId,
        bestow: bool,
    ) -> Result<Characteristics, String> {
        let definition = self.definition(card)?;
        if !bestow {
            return Ok(definition.characteristics.clone());
        }
        let (cost, _) = mtg_contracts::bestow_alternative(definition)
            .ok_or("missing printed bestow permission")?;
        Ok(
            mtg_contracts::BestowCastPlan::new(&definition.characteristics, cost.clone())?
                .characteristics(mtg_contracts::BestowForm::Aura),
        )
    }
    pub(super) fn announce_resolution_spell(&mut self, progress: CastOwner) -> Result<(), String> {
        let permission = progress.permission()?;
        if !self.resolution_offer_legal(&permission)? {
            return Err("no legal resolution cast".into());
        }
        self.begin_spell_announcement(SpellAnnouncement {
            bestow: false,
            player: permission.caster,
            original_object: permission.object,
            cast_from: Zone::Exile,
            completion: CastCompletion::Resolution {
                permission,
                progress,
            },
        })
    }

    pub(super) fn announce_combat_hand_spell(
        &mut self,
        frame: Box<Resolving>,
        object: ObjectId,
    ) -> Result<(), String> {
        let player = frame.spell.controller;
        if !self.hand_cast_candidates(&frame)?.contains(&object) {
            return Err("unavailable hand spell".into());
        }
        self.begin_spell_announcement(SpellAnnouncement {
            bestow: false,
            player,
            original_object: object,
            cast_from: Zone::Hand { owner: player },
            completion: CastCompletion::HandTrigger { frame },
        })
    }

    // CR 601.2a-i: one proposal constructor owns the move, fresh stack identity
    // and unfinished payment marker. No cast occurrence is emitted here.
    fn begin_spell_announcement(&mut self, announcement: SpellAnnouncement) -> Result<(), String> {
        let SpellAnnouncement {
            bestow,
            player,
            original_object,
            cast_from,
            completion,
        } = announcement;
        let card = original_object.card;
        self.check_stack_capacity(1)?;
        let alternative_cost = match &completion {
            CastCompletion::Priority => bestow.then_some(cascade::AlternativeCost::Bestow),
            CastCompletion::HandTrigger { .. } => Some(cascade::AlternativeCost::WithoutManaCost),
            CastCompletion::Resolution { permission, .. } => Some(permission.alternative_cost),
        };
        let total_cost = match alternative_cost {
            None => self.casting_cost(card, player, cast_from)?,
            Some(alternative) => {
                self.announcement_cost(card, player, cast_from, Some(alternative))?
            }
        };
        let id = StackId(self.state.rules.next_stack_id);
        let hand_cast_permission =
            match &completion {
                CastCompletion::HandTrigger { frame } => Some(Box::new(
                    self.hand_cast_permission(frame, original_object, id)?,
                )),
                _ => None,
            };
        self.state.rules.next_stack_id = self
            .state
            .rules
            .next_stack_id
            .checked_add(1)
            .ok_or("stack ID resource limit")?;
        self.record_event(Event::SpellProposed { card, stack_id: id })?;
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
        let effect_source_handles = self.capture_trigger_source_handles(source)?;
        let mut source_facts = self.object_before(source)?;
        if bestow {
            source_facts.characteristics = self.proposed_cast_characteristics(card, true)?;
        }
        self.state.rules.stack.push(StackInstance {
            hybrid_colors: vec![],
            cost_lock: None,
            selected_additional_costs: None,
            alternative_cost,
            hand_cast_permission,
            x: (alternative_cost.is_some() && self.definition(card)?.x_mana_cost)
                .then(Quantity::zero),
            death_controller: None,
            sacrificed: None,
            sacrificed_set: vec![],
            mode_subset: None,
            mode: None,
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
        });
        let cast = Casting {
            completion,
            sacrifices: vec![],
            origin: AnnouncementOrigin::Spell,
            spending: None,
            stack_id: id,
            original_object,
            prior_passes: self.state.rules.passes.clone(),
            mana_activations: vec![],
            undo_mana: false,
        };
        self.state.rules.frame = self.spell_announcement_frame(cast)?;
        self.state.rules.passes.clear();
        Ok(())
    }

    // All supported spells share modes/X/target routing. This selects a stage,
    // never grants a casting permission or changes a target filter's meaning.
    pub(super) fn spell_announcement_frame(&mut self, cast: Casting) -> Result<Frame, String> {
        let stack = self.top()?;
        if stack.id != cast.stack_id || !stack.kind.is_spell() || !cast.origin.is_spell() {
            return Err("spell announcement binding invariant".into());
        }
        let definition = self.definition(stack.source.card)?;
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
        if !definition
            .spell
            .as_ref()
            .ok_or("missing spell")?
            .modes
            .is_empty()
            && stack.mode.is_none()
            && stack.mode_subset.is_none()
        {
            return Ok(Frame::Modes { cast });
        }
        if !definition.characteristics.hybrid_mana.is_empty()
            && stack.alternative_cost.is_none()
            && stack.hybrid_colors.is_empty()
        {
            return Ok(Frame::HybridCost { cast });
        }
        if definition.x_mana_cost && stack.x.is_none() {
            return Ok(Frame::AnnounceX { cast });
        }
        Ok(
            match self.program()?.targets.first().map(|target| target.domain) {
                None => self.announcement_mana_frame(cast)?,
                Some(
                    TargetDomain::StackSpell
                    | TargetDomain::ControlledCreatureSpell
                    | TargetDomain::StackActivatedOrTriggeredAbility,
                ) => Frame::StackTargets { cast },
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
                    | TargetDomain::BlockingBattlefieldCreature
                    | TargetDomain::OwnGraveyardCreatureCard
                    | TargetDomain::OwnGraveyardCard,
                ) => Frame::Targets { cast },
            },
        )
    }
    /// CR 101.2/109.5/601.3/611.3a, 800.4j: "your turn" names the
    /// retained turn owner even when there is no active player. The current
    /// battlefield controller supplies "your"; ownership does not.
    /// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    pub(super) fn validate_cast_permission(&self, player: PlayerId) -> Result<(), String> {
        if !self.is_live(player) {
            return Err("departed player cannot cast spells".into());
        }
        if self.cast_is_prohibited(player)? {
            return Err("opponent cannot cast during source controller's turn".into());
        }
        Ok(())
    }
    pub(super) fn cast_is_prohibited(&self, player: PlayerId) -> Result<bool, String> {
        for source in self.state.rules.objects.values().filter(|o| {
            o.present_on_battlefield()
                && o.controller == self.state.rules.turn_owner
                && o.controller != player
        }) {
            if self
                .definition(source.id.card)?
                .static_abilities
                .iter()
                .any(|a| a.effect == StaticEffect::OpponentsCantCastDuringControllerTurn)
            {
                return Ok(true);
            }
        }
        Ok(false)
    }
    pub(super) fn validate_mana_activation(
        &self,
        player: PlayerId,
        object: ObjectId,
    ) -> Result<(), String> {
        if self.has_granted_land_mana(object)? {
            if self.reserved_mana_source(object) {
                return Err("reserved granted mana source".into());
            }
            return self.mixed_mana_choices(player, object).map(|_| ());
        }
        if self.is_permanent_mana(object) {
            return self.validate_permanent_mana_activation(player, object);
        }
        if self.reserved_mana_source(object) {
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

    /// Called only after binding a concrete intrinsic or granted ability.
    pub(super) fn commit_unrestricted_tap_mana(
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
        self.state.rules.stack.pop();
        if cast.origin.is_spell() {
            let o = self
                .state
                .rules
                .objects
                .get_mut(&cast.original_object.card)
                .ok_or("missing reversal object")?;
            o.id = cast.original_object;
            o.zone = stack.cast_from;
            o.controller = stack.controller;
            o.default_controller = stack.controller;
            o.tapped = false;
        }
        self.state.rules.pending = None;
        self.state.rules.priority = stack.controller;
        // CR 117.4: keeping a legal mana activation interrupts the old pass
        // round. Undoing all activations restores the pre-attempt live passes.
        let live = self.live_players();
        self.state.rules.passes = if undo_mana || cast.mana_activations.is_empty() {
            cast.prior_passes
                .into_iter()
                .filter(|p| live.contains(p))
                .collect()
        } else {
            vec![]
        };
        self.state.rules.frame = match cast.completion {
            CastCompletion::Priority => Frame::Priority,
            CastCompletion::HandTrigger { frame } => Frame::HandCast { frame, child: None },
            CastCompletion::Resolution {
                permission,
                progress,
            } => {
                if permission.object != cast.original_object
                    || permission.caster != stack.controller
                {
                    return Err("resolution cast reversal permission invariant".into());
                }
                progress.resume()
            }
        };
        self.record_event(Event::CastReversed {
            stack_id: cast.stack_id,
            reason: reason.into(),
        })?;
        Ok(())
    }
}
