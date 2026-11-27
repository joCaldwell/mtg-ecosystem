//! The complete structural invariant of a state. A decoded snapshot must
//! satisfy it before replay proves the rest; debug builds also check it after
//! every committed transaction.
use crate::engine::*;

impl RulesTxn {
    pub(crate) fn validate_stack_links(&self) -> Result<(), RuleError> {
        let rules = &self.state.rules;
        let mut new_root = true;
        let mut has_leaf = false;
        for (depth, frame) in rules
            .frames
            .iter()
            .chain(std::iter::once(&rules.frame))
            .enumerate()
        {
            if frame.parent().map(|link| link.0) != (!new_root).then(|| depth - 1) {
                return Err("invalid continuation stack link".into());
            }
            new_root = matches!(frame, Frame::Kernel { task } if task.is_leaf());
            if new_root {
                if has_leaf || depth == rules.frames.len() {
                    return Err("invalid kernel leaf marker".into());
                }
                has_leaf = true;
            }
        }
        Ok(())
    }
    pub(crate) fn validate_state(&self) -> Result<(), RuleError> {
        self.validate_stack_links()?;
        let rules = &self.state.rules;
        for (depth, frame) in rules
            .frames
            .iter()
            .chain(std::iter::once(&rules.frame))
            .enumerate()
        {
            if let Frame::Kernel { task } = frame {
                self.validate_kernel_task(task, depth)?;
            }
        }
        self.validate_match_state()?;
        self.validate_phasing_state()?;
        if self.state.rules.players.iter().any(|p| {
            p.life_frozen_until_next_turn
                && (p.status != PlayerStatus::Live
                    || !self
                        .prepared
                        .catalog
                        .definitions
                        .iter()
                        .any(|d| d.capabilities.contains(&Capability::ControllerLifeFreezeV1)))
        }) {
            return Err("life freeze requires a live player and a supporting catalog".into());
        }
        if rules.players.iter().any(|player| {
            player.poison_counters.is_negative() || player.energy_counters.is_negative()
        }) {
            return Err("snapshot player counter quantity invariant".into());
        }
        // CR 606.3: a loyalty use is stamped with the number of a turn this
        // game has had, which starts at 1, the same bound a mana receipt's
        // captured source row is held to.
        if rules.objects.values().any(|object| {
            object.loyalty_counters.is_negative()
                || object
                    .loyalty_used_turn
                    .is_some_and(|turn| turn == 0 || turn > rules.turn.number)
                || (object.zone != Zone::Battlefield
                    && (object.loyalty_counters > Quantity::zero()
                        || object.loyalty_used_turn.is_some()))
        }) {
            return Err("snapshot loyalty invariant".into());
        }
        let s = &self.state;
        // An invocation has one owner: its stack entry, or the detached entry
        // of an invocation the frame still resumes (CR 608.2m).
        if s.rules.detached.iter().any(|(id, entry)| {
            entry.id != *id
                || id.0 >= s.rules.next_stack_id
                || s.rules.stack.iter().any(|stacked| stacked.id == *id)
                || !s.rules.frame.holds(&s.rules.frames, *id)
        }) {
            return Err("snapshot detached invocation ownership invariant".into());
        }
        for entry in s.rules.detached.values() {
            self.validate_paid_chosen_cost(entry)?;
        }
        let mut unowned = false;
        s.rules.frame.for_each_held(&s.rules.frames, &mut |held| {
            if let frame::Held::Invocation(id) = held {
                unowned |= self.entry(id).is_err();
            }
        });
        if unowned {
            return Err("snapshot frame names an invocation without an owner".into());
        }
        if s.rules.objects.values().any(|object| {
            object.battlefield_entry_turn.is_some_and(|turn| {
                object.zone != Zone::Battlefield || turn == 0 || turn > s.rules.turn.number
            })
        }) {
            return Err("invalid battlefield entry turn".into());
        }
        for object in s
            .rules
            .objects
            .values()
            .filter(|o| o.id.face == CardFace::Back)
        {
            let layout = self.definition(object.id.card)?.layout;
            let card = self.copiable_definition(object.id)?;
            // CR 712.11-712.14, 715.3: only a modal face or an Adventure is
            // chosen as the card moves; a transforming card is never cast or
            // put onto the battlefield transformed here.
            if object.hideaway.is_some()
                || !matches!(
                    layout,
                    FaceLayout::ModalDoubleFaced | FaceLayout::Adventurer | FaceLayout::Split
                )
                || match object.zone {
                    Zone::Stack => card.spell.is_none(),
                    Zone::Battlefield => matches!(
                        card.characteristics.kind,
                        CardKind::Instant | CardKind::Sorcery
                    ),
                    _ => true,
                }
            {
                return Err("snapshot has an impossible back face for its zone".into());
            }
        }
        // CR 701.27a, 712.9-712.10, 712.18: only a transforming card's
        // permanent has transformed, and the face it shows is a permanent.
        for object in s.rules.objects.values().filter(|o| o.transforms > 0) {
            let definition = self.definition(object.id.card)?;
            if object.zone != Zone::Battlefield
                || object.hideaway.is_some()
                || object.id.face != CardFace::Front
                || !definition.transforms()
                || definition.face(object.face()).is_none_or(|face| {
                    matches!(
                        face.characteristics.kind,
                        CardKind::Instant | CardKind::Sorcery
                    )
                })
            {
                return Err("snapshot has an impossible transformed object".into());
            }
        }
        // CR 702.37c, 708.4, 708.9: only a physical card with morph is face
        // down, and only as a spell on the stack or a permanent; it shows its
        // front face and has neither transformed nor been exiled by hideaway.
        for object in s.rules.objects.values().filter(|o| o.face_down) {
            let spell = s
                .rules
                .stack
                .iter()
                .any(|entry| entry.kind.is_spell() && entry.source == object.id);
            if !(object.zone == Zone::Battlefield || (object.zone == Zone::Stack && spell))
                || object.hideaway.is_some()
                || object.adventure_player.is_some()
                || object.transforms != 0
                || object.id.face != CardFace::Front
                || s.rules.tokens.contains_key(&object.id.card)
                || self.definition(object.id.card)?.face_down.is_none()
            {
                return Err("snapshot has an impossible face-down object".into());
            }
        }
        // CR 715.3d: only an adventurer card exiled as a resolving Adventure
        // carries its controller's permission to play it.
        for object in s.rules.objects.values() {
            if let Some(player) = object.adventure_player
                && (object.zone != Zone::Exile
                    || object.hideaway.is_some()
                    || object.id.face != CardFace::Front
                    || player.0 as usize >= s.rules.players.len()
                    || self.definition(object.id.card)?.layout != FaceLayout::Adventurer)
            {
                return Err("snapshot has an impossible adventure permission".into());
            }
        }
        // CR 113.7a, 701.27f: an ability's face and transform baseline name
        // a face its source card has; a spell has neither.
        for entry in s.rules.stack.iter().chain(s.rules.detached.values()) {
            let definition = self.definition(entry.source.card)?;
            let invalid = if entry.kind.is_spell() {
                entry.source_face != CardFace::Front || entry.source_transforms != 0
            } else if definition.transforms() {
                entry.source.face != CardFace::Front
                    || self
                        .current(entry.source)
                        .is_ok_and(|live| live.transforms < entry.source_transforms)
            } else {
                entry.source_face != entry.source.face || entry.source_transforms != 0
            };
            if invalid || definition.face(entry.source_face).is_none() {
                return Err("snapshot has an impossible ability face".into());
            }
        }
        // CR 702.30a: a player has at most one upkeep per turn, and each
        // battlefield permanent records when it came under its controller's
        // control.
        if s.rules
            .players
            .iter()
            .any(|p| p.upkeeps > s.rules.turn.number)
            || s.rules.objects.values().any(|o| match o.control_upkeeps {
                // A forged controller outside the seats is a snapshot error.
                Some(n) => {
                    o.zone != Zone::Battlefield
                        || s.rules
                            .players
                            .get(o.controller.0 as usize)
                            .is_none_or(|p| n > p.upkeeps)
                }
                None => o.zone == Zone::Battlefield,
            })
        {
            return Err("invalid upkeep control history".into());
        }
        // CR 702.84a: only a battlefield incarnation is unearthed.
        if s.rules
            .objects
            .values()
            .any(|o| (o.unearthed || o.dashed) && o.zone != Zone::Battlefield)
        {
            return Err("unearthed or dashed object off the battlefield".into());
        }
        // Cleanup can remove land allowances before the next turn resets usage.
        // Replay proves the land count at the boundaries where it was incremented.
        if s.rules.players.len() != 4
            || s.knowledge.visibility.len() != 4
            || s.rules.turn_owner.0 >= 4
            || s.rules.turn.number == 0
            || s.rules.active.is_some_and(|p| !self.is_live(p))
            || s.rules.cast_history.iter().any(|h| {
                h.mana_value.is_negative()
                    || h.counter_placements.is_negative()
                    || h.counter_removals.is_negative()
                    || !h.any_cast && (h.exile_cast || h.mana_value != Quantity::zero())
                    || h.any_cast == h.spells.is_empty()
            })
            || s.rules.original_seating != (0..4).map(PlayerId).collect::<Vec<_>>()
            || s.rules.priority.0 >= 4
            || (!self.is_live(s.rules.priority) && !matches!(s.rules.frame, Frame::Draw))
            || s.rules.passes.len() > self.live_players().len()
            || s.rules.passes.iter().any(|p| !self.is_live(*p))
            || s.rules.passes.iter().collect::<BTreeSet<_>>().len() != s.rules.passes.len()
            || s.rules.cards.len() != self.prepared.config.cards.len() + s.rules.tokens.len()
            || s.rules.objects.len() > s.rules.cards.len()
            || s.rules.stack.len() > self.stack_limit()
        {
            return Err("snapshot seating/table invariant".into());
        }
        for setup in &self.prepared.config.cards {
            let card = s
                .rules
                .cards
                .get(&setup.id)
                .ok_or("snapshot missing card")?;
            if card.owner != setup.owner || card.oracle_id != setup.oracle_id {
                return Err("snapshot physical identity invariant".into());
            }
            let Some(o) = s.rules.objects.get(&setup.id) else {
                if self.is_live(setup.owner) {
                    return Err("snapshot missing live owner's object".into());
                }
                continue;
            };
            if !self.is_live(card.owner) {
                return Err("departed owner still has an object".into());
            }
            if o.id.card != setup.id
                || o.controller.0 >= 4
                || o.default_controller.0 >= 4
                || !self.is_live(o.controller)
                || (o.tapped && o.zone != Zone::Battlefield)
                || (o.deathtouch_damage && (o.zone != Zone::Battlefield))
            {
                return Err("snapshot object invariant".into());
            }
            if let Zone::Hand { owner } | Zone::Graveyard { owner } = o.zone
                && owner != card.owner
            {
                return Err("snapshot zone-owner invariant".into());
            }
            // CR 304.4, 307.4: no instant or sorcery is ever a permanent.
            if o.zone == Zone::Battlefield
                && matches!(
                    self.copiable_definition(o.id)
                        .map_err(|_| "snapshot battlefield definition")?
                        .characteristics
                        .kind,
                    CardKind::Instant | CardKind::Sorcery
                )
            {
                return Err("snapshot instant or sorcery on the battlefield".into());
            }
        }
        self.validate_attachment_state()?;
        self.validate_control_state()?;
        self.validate_hideaway_links()?;
        self.validate_tokens_state()?;
        self.validate_counter_placement_state()?;
        self.validate_mana_uses()?;
        self.validate_activation_uses()?;
        self.validate_turn_history()?;
        self.validate_resolution_triggers()?;
        self.validate_trigger_state()?;
        self.validate_library_position_memory()?;
        self.validate_effect_state()?;
        self.validate_choices_state()?;
        self.validate_commander_state()?;
        self.validate_actions_state()?;

        for p in &s.rules.players {
            if (p.status == PlayerStatus::Left) != p.last_known.is_some()
                || p.last_known.as_ref().is_some_and(|last| {
                    last.life != p.life
                        || last.mana != p.mana
                        || last.restricted_mana != p.restricted_mana
                })
            {
                return Err("snapshot departed-player history invariant".into());
            }
            if p.mana.values().any(|n| n.is_negative())
                || !restricted_mana::valid_buckets(&p.restricted_mana)
            {
                return Err("snapshot invalid mana pool".into());
            }
        }
        let ids: BTreeSet<_> = s.rules.stack.iter().map(|entry| entry.id).collect();
        let sources: BTreeSet<_> = s
            .rules
            .stack
            .iter()
            .filter(|entry| entry.kind.is_spell())
            .map(|entry| entry.source.card)
            .collect();
        if ids.len() != s.rules.stack.len()
            || sources.len()
                != s.rules
                    .stack
                    .iter()
                    .filter(|entry| entry.kind.is_spell())
                    .count()
            || !s.rules.stack.windows(2).all(|w| w[0].id < w[1].id)
        {
            return Err("snapshot duplicate or unordered stack identity".into());
        }
        for (index, stack) in s.rules.stack.iter().enumerate() {
            if stack.id.0 >= s.rules.next_stack_id
                || !self.is_live(stack.controller)
                || (stack.kind.is_spell() && self.current(stack.source)?.zone != Zone::Stack)
            {
                return Err("snapshot stack identity invariant".into());
            }
            self.validate_stack_bindings(stack)?;
            let program = self.program_for(stack)?;
            if stack.announced_targets.len() != 4
                || stack
                    .announced_targets
                    .iter()
                    .any(|a| a.len() != stack.targets.len())
            {
                return Err("snapshot target announcement invariant".into());
            }
            let incomplete_top = index + 1 == s.rules.stack.len()
                && (matches!(
                    self.underlying_mana_frame(&s.rules.frame)?,
                    Frame::TriggerTargets { .. }
                        | Frame::ChooseModes { .. }
                        | Frame::AdditionalCosts { .. }
                        | Frame::HybridCost { .. }
                        | Frame::AnnounceX { .. }
                        | Frame::EntityTargets { .. }
                        | Frame::Targets { .. }
                        | Frame::StackTargets { .. }
                        | Frame::PlayerTargets { .. }
                        | Frame::Mana { .. }
                        | Frame::SpendMana { .. }
                        | Frame::SacrificeCost { .. }
                        | Frame::ChosenObjectCost { .. }
                        | Frame::Payment { .. }
                ) || (matches!(
                    self.underlying_mana_frame(&s.rules.frame)?,
                    Frame::Finished { .. } | Frame::Fault { .. }
                ) && stack.paid.is_none()
                    && !self.is_spell_copy(stack)));
            // CR 601.2c, 601.2f: targets are bound before the cost locks, so
            // a locked entry keeps them even when the game ends unpaid.
            if stack.cost_lock.is_some() && !self.announced_target_count_valid(program, stack) {
                return Err("snapshot locked spell target invariant".into());
            }
            if !incomplete_top
                && (!self.announced_target_count_valid(program, stack)
                    || !self.stack_entry_complete(stack))
            {
                return Err("snapshot completed lower/top cast invariant".into());
            }
        }
        let stack_cards: BTreeSet<_> = s
            .rules
            .stack
            .iter()
            .filter(|entry| entry.kind.is_spell())
            .map(|entry| entry.source.card)
            .collect();
        if s.rules
            .objects
            .values()
            .any(|o| (o.zone == Zone::Stack) != stack_cards.contains(&o.id.card))
        {
            return Err("snapshot stack/zone invariant".into());
        }
        self.validate_battlefield_entry()?;
        self.validate_graveyard_replacement()?;
        self.validate_sacrifice_state()?;
        self.validate_nested_mana_frame(&s.rules.frame)?;
        // Paid mana damage can retain the resolving instruction even after its
        // payer departs and the activation no longer has a live mana window.
        if let Frame::ResolutionPayment { frame, payment } =
            frame::retained_effect_frame(&self.state.rules.frames, &s.rules.frame)
        {
            self.validate_resolution_payment(frame, payment)?;
        }
        if let Frame::TurnFaceUp { payment } = self.underlying_mana_frame(&s.rules.frame)? {
            self.validate_face_up_payment(payment)?;
        }
        match self.underlying_mana_frame(&s.rules.frame)? {
            Frame::ChooseModes { cast }
            | Frame::AdditionalCosts { cast }
            | Frame::HybridCost { cast }
            | Frame::AnnounceX { cast }
            | Frame::EntityTargets { cast }
            | Frame::Targets { cast }
            | Frame::StackTargets { cast }
            | Frame::PlayerTargets { cast }
            | Frame::Mana { cast }
            | Frame::SpendMana { cast }
            | Frame::SacrificeCost { cast }
            | Frame::ChosenObjectCost { cast, .. }
            | Frame::Payment { cast } => {
                let top = self.top()?;
                if (!cast.sacrifices.is_empty()
                    && !matches!(
                        self.underlying_mana_frame(&s.rules.frame)?,
                        Frame::SacrificeCost { .. }
                    ))
                    || cast.origin.is_spell() != top.kind.is_spell()
                    || cast.stack_id != top.id
                    || cast.original_object.card != top.source.card
                    || if cast.origin.is_spell() {
                        cast.original_object.incarnation.checked_add(1)
                            != Some(top.source.incarnation)
                    } else {
                        cast.original_object != top.source
                    }
                    || top.paid.is_some()
                    || (cast.revealed_hand_consumed
                        && !(cast.origin.is_spell()
                            && top.cast_from
                                == (Zone::Hand {
                                    owner: s.rules.cards[&top.source.card].owner,
                                })))
                {
                    return Err("snapshot casting frame invariant".into());
                }
                let sources: BTreeSet<_> = cast.mana_activations.iter().map(|a| a.object).collect();
                // Each kept receipt's own invariant names what it rejected.
                for a in &cast.mana_activations {
                    if let Some(p) = &a.permanent {
                        self.validate_mana_condition_receipt(a.player, a.object, p)?;
                    }
                }
                if sources.len() != cast.mana_activations.len()
                    || cast.mana_activations.len() > s.rules.cards.len()
                    || cast.mana_activations.iter().any(|a| {
                        a.player != top.controller
                            || (a.permanent.is_none() && a.amount != Quantity::from(1))
                    })
                    || (matches!(
                        self.underlying_mana_frame(&s.rules.frame)?,
                        Frame::ChooseModes { .. }
                            | Frame::AdditionalCosts { .. }
                            | Frame::HybridCost { .. }
                            | Frame::AnnounceX { .. }
                            | Frame::EntityTargets { .. }
                            | Frame::Targets { .. }
                            | Frame::PlayerTargets { .. }
                            | Frame::StackTargets { .. }
                    ) && (!cast.mana_activations.is_empty() || cast.undo_mana))
                    || (matches!(
                        self.underlying_mana_frame(&s.rules.frame)?,
                        Frame::Mana { .. }
                    ) && cast.undo_mana)
                {
                    return Err("snapshot casting mana receipt invariant".into());
                }
                let stage = self.underlying_mana_frame(&s.rules.frame)?;
                if cast.delve.is_some() || cast.tap_payment.is_some() {
                    if !matches!(
                        stage,
                        Frame::SpendMana { .. }
                            | Frame::SacrificeCost { .. }
                            | Frame::ChosenObjectCost { .. }
                    ) {
                        return Err("snapshot spell payment stage invariant".into());
                    }
                    self.mana_due(top, cast)?;
                }
                let target_stage = matches!(
                    stage,
                    Frame::EntityTargets { .. }
                        | Frame::Targets { .. }
                        | Frame::PlayerTargets { .. }
                        | Frame::StackTargets { .. }
                );
                if (target_stage
                    || matches!(
                        stage,
                        Frame::ChooseModes { .. }
                            | Frame::AdditionalCosts { .. }
                            | Frame::HybridCost { .. }
                            | Frame::AnnounceX { .. }
                    ))
                    && self.bound_target_count(top) != 0
                {
                    return Err("snapshot announcement target invariant".into());
                }
                if matches!(
                    self.underlying_mana_frame(&s.rules.frame)?,
                    Frame::Mana { .. }
                        | Frame::SpendMana { .. }
                        | Frame::SacrificeCost { .. }
                        | Frame::ChosenObjectCost { .. }
                        | Frame::Payment { .. }
                ) && !self.announced_target_count_valid(self.program()?, top)
                {
                    return Err("snapshot payment target invariant".into());
                }
                if let Frame::ChosenObjectCost { selected, .. } = stage {
                    self.validate_chosen_cost_frame(top, cast, selected)?;
                }
            }
            Frame::Resolve { frame } | Frame::ResolutionChoice { frame, .. } => {
                self.validate_resolving(frame)?
            }
            _ => {}
        }
        if let Some(pending) = &s.rules.pending {
            if pending.decision_id >= s.rules.revision || s.audit.journal.last() != Some(pending) {
                return Err("snapshot pending-input invariant".into());
            }
            if !matches!(s.rules.frame, Frame::Fault { .. }) {
                self.validate_pending(pending)?;
            }
        }
        self.validate_legend_state()?;
        Ok(self.validate_combat_state()?)
    }
}
