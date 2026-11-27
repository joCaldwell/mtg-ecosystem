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
    face: CardFace,
    /// CR 702.37c: cast face down using its morph ability.
    face_down: bool,
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
        face: CardFace,
        face_down: bool,
    ) -> Result<SpellAnnouncement, String> {
        if !matches!(self.state.rules.frame, Frame::Priority) || self.state.rules.priority != player
        {
            return Err("spell proposal requires its ordinary priority owner".into());
        }
        self.validate_cast_permission(player)?;
        let object = self.state.rules.objects.get(&card).ok_or("invalid cast")?;
        if face_down {
            return self.face_down_announcement(player, object);
        }
        let definition = self.face_definition(card, face)?;
        // CR 702.34a: flashback permits casting from its owner's graveyard.
        // CR 715.3d: an exiled Adventure's controller may cast its card, but
        // not as an Adventure.
        if !(object.zone == (Zone::Hand { owner: player })
            || (object.zone == (Zone::Command { owner: player })
                && self.state.rules.commanders.history(card).is_some())
            || (object.zone == (Zone::Graveyard { owner: player })
                && (definition.flashback.is_some()
                    || definition.retrace
                    || definition.escape.is_some()))
            || (object.zone == Zone::Exile
                && object.adventure_player == Some(player)
                && face == CardFace::Front))
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
            && !self.has_flash_permission(object, face, false, player)?
        {
            return Err("main-phase spell casting requires active-player main-phase priority and empty stack".into());
        }
        if let Some(reason) =
            self.spell_proposal_obstacle(program, player, &definition.characteristics)
        {
            return Err(reason.into());
        }
        // CR 702.81a, 601.2b: a retrace cast needs a land card to discard.
        if definition.retrace
            && object.zone == (Zone::Graveyard { owner: player })
            && !self.state.rules.objects.values().any(|o| {
                o.zone == (Zone::Hand { owner: player })
                    && !self.state.rules.tokens.contains_key(&o.id.card)
                    && self
                        .characteristics(o.id)
                        .is_ok_and(|c| c.has_type(CardKind::Land))
            })
        {
            return Err("retrace requires a land card to discard".into());
        }
        // CR 702.138a, 601.2b: an escape cast needs enough other cards to exile.
        if let Some(escape) = &definition.escape
            && object.zone == (Zone::Graveyard { owner: player })
            && self
                .state
                .rules
                .objects
                .values()
                .filter(|o| {
                    o.zone == (Zone::Graveyard { owner: player })
                        && o.id.card != card
                        && !self.state.rules.tokens.contains_key(&o.id.card)
                })
                .count()
                < escape.exile_other as usize
        {
            return Err("escape requires other graveyard cards to exile".into());
        }
        if let Some(modal) = &definition.modal_spell
            && !self.modal_has_selection(modal, player, &definition.characteristics, None)
        {
            return Err("no legal mode selection for this proposal".into());
        }
        Ok(SpellAnnouncement {
            player,
            original_object: object.id,
            cast_from: object.zone,
            face,
            face_down: false,
        })
    }

    /// CR 702.37a-d, 708.4: a card with morph may be cast face down from any
    /// zone it could normally be cast from (here its owner's hand, or the
    /// command zone for a commander). Timing, prohibitions and targets are
    /// those of the face-down 2/2 creature spell, not the printed card.
    fn face_down_announcement(
        &self,
        player: PlayerId,
        object: &Object,
    ) -> Result<SpellAnnouncement, String> {
        let card = object.id.card;
        let printed = self.face_definition(card, CardFace::Front)?;
        if printed.face_down.is_none()
            || self.state.rules.tokens.contains_key(&card)
            || self.state.rules.cards[&card].owner != player
            || !(object.zone == (Zone::Hand { owner: player })
                || (object.zone == (Zone::Command { owner: player })
                    && self.state.rules.commanders.history(card).is_some()))
        {
            return Err("invalid or unsupported face-down cast".into());
        }
        // CR 708.5, 903.10a: commander combat damage is tracked publicly by
        // physical card, so a commander cast face down from a hand would be
        // identified by its tally. From the command zone it is already
        // public which card left.
        if object.zone == (Zone::Hand { owner: player })
            && self.state.rules.commanders.history(card).is_some()
        {
            return Err("unsupported face-down cast of a commander from a hand".into());
        }
        let face_down = face_down::face_down_definition();
        let program = face_down
            .spell
            .as_ref()
            .ok_or("missing face-down program")?;
        if program.timing == SpellTiming::MainPhase
            && !self.main_phase_action(player)
            && !self.has_flash_permission(object, CardFace::Front, true, player)?
        {
            return Err("main-phase spell casting requires active-player main-phase priority and empty stack".into());
        }
        if let Some(reason) =
            self.spell_proposal_obstacle(program, player, &face_down.characteristics)
        {
            return Err(reason.into());
        }
        Ok(SpellAnnouncement {
            player,
            original_object: object.id,
            cast_from: object.zone,
            face: CardFace::Front,
            face_down: true,
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
        // A spell names no battlefield source, so no target excludes one.
        if !self.has_target_domain(program, player, source, None) {
            return Some("no legal target for this proposal");
        }
        None
    }

    /// Quote an ordinary front-face cast for its priority holder using the
    /// same cost construction that will lock at CR 601.2f, including a
    /// graveyard cast's flashback or escape cost or retrace's mana cost (CR
    /// 702.34a, 702.81a, 702.138a). Optional and hybrid costs are left to
    /// their respective announcement choices.
    pub fn quote_cast_cost(
        &self,
        viewer: PlayerId,
        card: &ObjectHandle,
        x: Option<&Quantity>,
    ) -> Result<ManaCost, String> {
        let object = self.resolve_handle(viewer, card)?;
        let proposal =
            self.priority_spell_announcement(viewer, object.card, CardFace::Front, false)?;
        let source = ObjectId {
            face: proposal.face,
            ..proposal.original_object
        };
        self.casting_cost(source, viewer, proposal.cast_from, x)
    }

    /// CR 107.4e-f, 601.2f: the same quote with one announced alternative
    /// per hybrid symbol, which the lock adds before increases and
    /// reductions apply.
    pub fn quote_hybrid_cast_cost(
        &self,
        viewer: PlayerId,
        card: &ObjectHandle,
        x: Option<&Quantity>,
        hybrid: &[HybridPayment],
    ) -> Result<ManaCost, String> {
        let object = self.resolve_handle(viewer, card)?;
        let proposal =
            self.priority_spell_announcement(viewer, object.card, CardFace::Front, false)?;
        let source = ObjectId {
            face: proposal.face,
            ..proposal.original_object
        };
        let lock = self.cost_lock_inputs(source, viewer, proposal.cast_from)?;
        self.construct_spell_cost(
            source,
            proposal.cast_from,
            x,
            &[],
            (!hybrid.is_empty()).then_some(hybrid),
            &lock,
        )
    }

    /// CR 118.9, 601.2f: quote the printed alternative cost (evoke, CR
    /// 702.74a, or dash, CR 702.109a) of the same front-face proposal from
    /// hand or the command zone, with the commander tax, increases and
    /// reductions that would lock with it. Graveyard routes (flashback,
    /// retrace, escape) are the proposal's own cost: see `quote_cast_cost`.
    pub fn quote_alternative_cast_cost(
        &self,
        viewer: PlayerId,
        card: &ObjectHandle,
    ) -> Result<Option<(String, ManaCost)>, String> {
        let object = self.resolve_handle(viewer, card)?;
        let proposal =
            self.priority_spell_announcement(viewer, object.card, CardFace::Front, false)?;
        if !matches!(proposal.cast_from, Zone::Hand { .. } | Zone::Command { .. }) {
            return Ok(None);
        }
        let source = ObjectId {
            face: proposal.face,
            ..proposal.original_object
        };
        let Some(alternative) = self.copiable_definition(source)?.alternative_cost.as_ref() else {
            return Ok(None);
        };
        let lock = self.cost_lock_inputs(source, viewer, proposal.cast_from)?;
        let cost = self.construct_spell_cost(
            source,
            proposal.cast_from,
            None,
            std::slice::from_ref(&alternative.identity),
            None,
            &lock,
        )?;
        Ok(Some((alternative.identity.clone(), cost)))
    }

    /// CR 702.37c, 708.4, 601.2f: quote a face-down cast of `card`: {3} as
    /// the alternative cost of the face-down creature spell, with the
    /// commander tax, increases and reductions that would lock with it.
    pub fn quote_face_down_cast_cost(
        &self,
        viewer: PlayerId,
        card: &ObjectHandle,
    ) -> Result<ManaCost, String> {
        let object = self.resolve_handle(viewer, card)?;
        let proposal =
            self.priority_spell_announcement(viewer, object.card, CardFace::Front, true)?;
        // Price the face-down characteristics the spell will have, on a fork.
        let mut fork = self.fork();
        fork.rules_mut()
            .objects
            .get_mut(&object.card)
            .ok_or("missing face-down quote card")?
            .face_down = true;
        fork.casting_cost(proposal.original_object, viewer, proposal.cast_from, None)
    }

    pub(super) fn validate_priority_spell_cast(
        &self,
        player: PlayerId,
        card: CardId,
        face: CardFace,
    ) -> Result<(), String> {
        self.priority_spell_announcement(player, card, face, false)
            .map(|_| ())
    }

    /// CR 702.37a: whether `player` may cast `card` face down now.
    pub(super) fn validate_face_down_cast(
        &self,
        player: PlayerId,
        card: CardId,
    ) -> Result<(), String> {
        self.priority_spell_announcement(player, card, CardFace::Front, true)
            .map(|_| ())
    }

    pub(crate) fn announce_face_down_spell(
        &mut self,
        player: PlayerId,
        card: CardId,
    ) -> Result<(), String> {
        let announcement = self.priority_spell_announcement(player, card, CardFace::Front, true)?;
        self.begin_spell_announcement(announcement)
    }

    pub(crate) fn announce_priority_spell(
        &mut self,
        player: PlayerId,
        card: CardId,
        face: CardFace,
    ) -> Result<(), String> {
        let announcement = self.priority_spell_announcement(player, card, face, false)?;
        self.begin_spell_announcement(announcement)
    }

    // CR 601.2a-i: one proposal constructor owns the move, fresh stack identity
    // and unfinished payment marker. No cast occurrence is emitted here.
    fn begin_spell_announcement(&mut self, announcement: SpellAnnouncement) -> Result<(), String> {
        let SpellAnnouncement {
            player,
            original_object,
            cast_from,
            face,
            face_down,
        } = announcement;
        let card = original_object.card;
        self.check_stack_capacity(1)?;
        // CR 702.37c, 708.4: a face-down spell's cost is priced from its
        // face-down characteristics once it is on the stack.
        let printed_cost = if face_down {
            None
        } else {
            Some(self.casting_cost(
                ObjectId {
                    face,
                    ..original_object
                },
                player,
                cast_from,
                None,
            )?)
        };
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
                    face,
                    face_down,
                },
            )?
            .successor;
        let total_cost = match printed_cost {
            Some(cost) => cost,
            None => self.casting_cost(source, player, cast_from, None)?,
        };
        // CR 400.2, 708.4: a face-down departure could have been any hand
        // card, so it consumed one copy of every known identity; a reversal
        // restores none of them rather than naming this card.
        let revealed_hand_consumed =
            !face_down && self.known_hand_count(original_object.card) < known_before;
        let effect_source_handles = self.capture_trigger_source_handles(source)?;
        let source_facts = self.object_before(source)?;
        let entry = StackInstance {
            target_triggers: vec![],
            mode_mask: None,
            hybrid_colors: vec![],
            cost_lock: None,
            selected_additional_costs: None,
            x: None,
            death_controller: None,
            sacrificed: None,
            sacrificed_set: vec![],
            chosen_payment: vec![],
            effect_source_handles,
            source_facts: Some(source_facts),
            entity_targets: vec![],
            announced_entity_targets: vec![],
            stack_targets: vec![],
            announced_stack_targets: vec![],
            kind: StackKind::Spell,
            source_face: CardFace::Front,
            source_transforms: 0,
            player_targets: vec![],
            id,
            source,
            controller: player,
            targets: vec![],
            announced_targets: vec![vec![]; 4],
            paid: None,
            mana_uncounterable: false,
            total_cost,
            cast_from,
        };
        self.rules_mut().stack.push(entry);
        let cast = Casting {
            sacrifices: vec![],
            origin: AnnouncementOrigin::Spell,
            spending: None,
            delve: None,
            tap_payment: None,
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
        let definition = self.stack_definition(stack)?;
        if definition.modal_spell.is_some() && stack.mode_mask.is_none() {
            return Ok(Frame::ChooseModes { cast });
        }
        definition.spell.as_ref().ok_or("missing spell")?;
        if !self.optional_cost_declarations(stack)?.is_empty()
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
            // CR 115.1, 120.3: planeswalker cards are not admitted, so the domain's
            // inhabitants are players.
            Some(
                TargetDomain::OpponentPlayer
                | TargetDomain::AnyPlayer
                | TargetDomain::PlayerOrPlaneswalker,
            ) => Frame::PlayerTargets { cast },
            Some(
                TargetDomain::ControlledBattlefieldCreature
                | TargetDomain::ControlledBattlefieldPermanent
                | TargetDomain::BattlefieldPermanent
                | TargetDomain::BattlefieldCreature
                | TargetDomain::CreatureOrPlaneswalker
                | TargetDomain::BlockingBattlefieldCreature
                | TargetDomain::OwnGraveyardCreatureCard
                | TargetDomain::GraveyardCard { .. }
                | TargetDomain::OwnGraveyardCard,
            ) => Frame::Targets { cast },
        })
    }
    /// CR 702.34a: a spell whose flashback cost was paid (the one way a spell
    /// is cast from a graveyard) is exiled instead of moving anywhere else as
    /// it leaves the stack. A copy was never cast and keeps `destination`.
    /// CR 715.3d: a spell cast from exile with an Adventure's permission is
    /// its card's normal characteristics.
    pub(super) fn adventurer_cast_from_exile(&self, entry: &StackInstance) -> Result<bool, String> {
        Ok(entry.kind.is_spell()
            && entry.cast_from == Zone::Exile
            && !self.is_spell_copy(entry)
            && entry.source.face == CardFace::Front
            && self.definition(entry.source.card)?.layout == FaceLayout::Adventurer)
    }
    /// CR 715.3b: the spell is on the stack as an Adventure.
    /// A copy is never a card and is not exiled (CR 707.10, 715.3c-d).
    pub(super) fn cast_as_adventure(&self, entry: &StackInstance) -> Result<bool, String> {
        Ok(entry.kind.is_spell()
            && !self.is_spell_copy(entry)
            && entry.source.face == CardFace::Back
            && self.definition(entry.source.card)?.layout == FaceLayout::Adventurer)
    }
    pub(super) fn stack_departure(&self, entry: &StackInstance, destination: Zone) -> Zone {
        // CR 702.81a: a retrace spell has no departure replacement.
        if entry.kind.is_spell()
            && matches!(entry.cast_from, Zone::Graveyard { .. })
            && self
                .stack_definition(entry)
                .is_ok_and(|d| d.flashback.is_some())
        {
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
            // CR 733.1: the card returns as it was, face up and unrevealed.
            o.face_down = false;
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
