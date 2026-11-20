//! Pure optional-life entry, CR118.4, 305.1, 614.12 (2026-09-25).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
//! Choices precede every debit/entry. Only the exact originating batch can
//! obtain ephemeral authority; neither snapshots nor callers supply that proof.
use super::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "origin", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum EntryOrigin {
    LinkedPlay {
        progress: Box<LinkedPlayProgress>,
        object: ObjectId,
        receipt: HideawayLink,
    },
    LandPlay {
        player: PlayerId,
        decision_id: u64,
    },
    Search {
        frame: Resolving,
    },
    Cascade {
        progress: Box<CascadeProgress>,
    },
}
impl EntryOrigin {
    pub(super) fn player(&self) -> PlayerId {
        match self {
            Self::LandPlay { player, .. } => *player,
            Self::Search { frame } => frame.spell.controller,
            Self::Cascade { progress } => progress.frame.spell.controller,
            Self::LinkedPlay { progress, .. } => progress.frame.spell.controller,
        }
    }
}

// Private, nonserializable, immutable batch-local proof. Constructed ONLY by
// prepare_entry_batch after complete cause/group equality and pre-debit checks.
pub(super) struct PreparedEntryAuthority {
    player: PlayerId,
    put_by_effect: bool,
    force_tapped: bool,
    entries: BTreeMap<ObjectId, Option<bool>>,
}
impl PreparedEntryAuthority {
    pub(super) fn land_play(&self, player: PlayerId) -> bool {
        !self.put_by_effect && self.player == player
    }
    pub(super) fn movement(
        &self,
        object: ObjectId,
        intended: Option<PlayerId>,
        forced: bool,
    ) -> Result<(PlayerId, bool), String> {
        let paid = self.entries.get(&object).ok_or("foreign prepared entry")?;
        if forced != self.force_tapped || intended != self.put_by_effect.then_some(self.player) {
            return Err("prepared entry producer/controller mismatch".into());
        }
        Ok((self.player, *paid == Some(false)))
    }
}
impl Game {
    pub(super) fn optional_entry_cost(
        &self,
        object: ObjectId,
    ) -> Result<Option<(u32, &Quantity)>, String> {
        let d = self.copiable_definition(object)?;
        let mut found = None;
        for (index, a) in d.static_abilities.iter().enumerate() {
            if let StaticEffect::EntersTappedUnlessPayLife { amount } = &a.effect {
                if found.is_some()
                    || d.static_abilities.len() != 1
                    || amount.is_negative()
                    || a.functioning_zone != FunctioningZone::Battlefield
                {
                    return Err("unsupported optional entry composition".into());
                }
                found = Some((
                    u32::try_from(index).map_err(|_| "entry static index limit")?,
                    amount,
                ));
            }
        }
        Ok(found)
    }
    fn entry_frame(&self) -> Result<(&EntryOrigin, &[ObjectId], &[bool]), String> {
        let Frame::LandEntry {
            origin,
            objects,
            choices,
        } = &self.state.rules.frame
        else {
            return Err("no land entry transaction".into());
        };
        Ok((origin, objects, choices))
    }
    fn entry_offers(&self, objects: &[ObjectId]) -> Result<Vec<(ObjectId, u32, Quantity)>, String> {
        let mut offers = vec![];
        for object in objects {
            if let Some((ability, amount)) = self.optional_entry_cost(*object)? {
                offers.push((*object, ability, amount.clone()));
            }
        }
        Ok(offers)
    }
    fn validate_entry_plan(&self) -> Result<(), String> {
        let (origin, objects, choices) = self.entry_frame()?;
        let player = origin.player();
        if !self.is_live(player)
            || objects.is_empty()
            || objects.len() > self.state.rules.cards.len()
            || !objects.windows(2).all(|w| w[0] < w[1])
        {
            return Err("invalid entry player/source set".into());
        }
        for object in objects {
            self.current(*object)?;
        }
        match origin {
            EntryOrigin::LandPlay { decision_id, .. } => {
                if objects.len() != 1 || *decision_id >= self.state.rules.revision {
                    return Err("invalid entry landplay origin".into());
                }
                // Reuse the original special-action legality without granting
                // priority or consuming its pending boolean on the real state.
                let mut validation = self.fork();
                validation.state.rules.frame = Frame::Priority;
                validation.validate_land_play(player, objects[0].card)?;
            }
            EntryOrigin::Search { frame } => {
                self.validate_resolving(frame)?;
                self.validate_entry_search(frame, objects)?;
            }
            EntryOrigin::LinkedPlay {
                progress,
                object,
                receipt,
            } => {
                self.validate_linked_play(progress, None)?;
                if !progress.receipt_matches(receipt, *object)
                    || objects != [*object]
                    || !self.linked_land_playable(player, *object)?
                {
                    return Err("invalid linked land play".into());
                }
            }
            EntryOrigin::Cascade { progress } => {
                self.validate_cascade_state()?;
                if objects.len() != 1 || !self.cascade_land_available(progress, objects[0])? {
                    return Err("invalid cascade land entry origin".into());
                }
            }
        }
        let offers = self.entry_offers(objects)?;
        if (offers.is_empty()
            && !matches!(
                origin,
                EntryOrigin::Cascade { .. } | EntryOrigin::LinkedPlay { .. }
            ))
            || choices.len() > offers.len()
        {
            return Err("invalid entry choice prefix".into());
        }
        self.entry_affordable(player, &offers, choices)
    }
    fn entry_affordable(
        &self,
        player: PlayerId,
        offers: &[(ObjectId, u32, Quantity)],
        choices: &[bool],
    ) -> Result<(), String> {
        let mut total = Quantity::zero();
        for ((_, _, amount), paid) in offers.iter().zip(choices) {
            if *paid {
                total.add(amount);
            }
        }
        // CR118.4: zero is payable even with synthetic negative life. Positive
        // costs permit equality; all selected costs reserve ONE common balance.
        if !self.state.rules.players[player.0 as usize].can_pay_life(&total) {
            return Err("unaffordable combined entry life cost".into());
        }
        Ok(())
    }
    pub(super) fn begin_land_entry(
        &mut self,
        origin: EntryOrigin,
        mut objects: Vec<ObjectId>,
    ) -> Result<bool, String> {
        objects.sort();
        if !matches!(
            origin,
            EntryOrigin::Cascade { .. } | EntryOrigin::LinkedPlay { .. }
        ) && self.entry_offers(&objects)?.is_empty()
        {
            return Ok(false);
        }
        self.state.rules.frame = Frame::LandEntry {
            origin,
            objects,
            choices: vec![],
        };
        self.validate_entry_plan()?;
        Ok(true)
    }
    pub(super) fn land_entry_offer(&self) -> Result<Option<(PlayerId, ObjectId, u32)>, String> {
        if !matches!(self.state.rules.frame, Frame::LandEntry { .. }) {
            return Ok(None);
        }
        self.validate_entry_plan()?;
        let (origin, objects, choices) = self.entry_frame()?;
        Ok(self
            .entry_offers(objects)?
            .get(choices.len())
            .map(|(object, ability, _)| (origin.player(), *object, *ability)))
    }
    pub(super) fn land_entry_request(&self) -> Result<Option<Request>, String> {
        Ok(self
            .land_entry_offer()?
            .map(|(player, _, _)| Request::OptionalEffect {
                decision_id: self.state.rules.revision,
                player,
            }))
    }
    pub(super) fn validate_land_entry_response(
        &self,
        player: PlayerId,
        response: &Response,
    ) -> Result<(), String> {
        self.validate_entry_plan()?;
        let (origin, objects, choices) = self.entry_frame()?;
        let Response::OptionalEffect { perform } = response else {
            return Err("invalid entry response".into());
        };
        let offers = self.entry_offers(objects)?;
        if origin.player() != player || choices.len() >= offers.len() {
            return Err("wrong entry chooser/phase".into());
        }
        let mut proposed = choices.to_vec();
        proposed.push(*perform);
        self.entry_affordable(player, &offers, &proposed)
    }
    fn entry_cause(&self) -> Result<BatchCause, String> {
        let (origin, objects, _) = self.entry_frame()?;
        match origin {
            EntryOrigin::LandPlay {
                player,
                decision_id,
            } => Ok(BatchCause::LandPlay {
                player: *player,
                decision_id: *decision_id,
                source: *objects.first().ok_or("empty entry")?,
            }),
            EntryOrigin::Search { frame } => {
                Ok(Self::staple_cause(frame, self.entry_search_block(frame)?))
            }
            EntryOrigin::LinkedPlay { progress, .. } => {
                Ok(Self::wipe_cause(&progress.frame, progress.source_block))
            }
            EntryOrigin::Cascade { progress } => {
                Ok(Self::wipe_cause(&progress.frame, progress.source_block))
            }
        }
    }
    fn entry_group(&self) -> Result<EventGroup, String> {
        let (origin, objects, choices) = self.entry_frame()?;
        let offers = self.entry_offers(objects)?;
        if choices.len() != offers.len() {
            return Err("incomplete entry decisions".into());
        }
        let player = origin.player();
        let mut children = vec![];
        for ((_, _, amount), paid) in offers.iter().zip(choices) {
            if *paid && *amount > Quantity::zero() {
                children.push(EventGroup::Occurrence {
                    proposed: ProposedEvent::LoseLife {
                        player,
                        amount: amount.clone(),
                    },
                });
            }
        }
        children.push(EventGroup::Simultaneous {
            occurrences: objects
                .iter()
                .map(|object| match origin {
                    EntryOrigin::LandPlay { .. } | EntryOrigin::LinkedPlay { .. } => {
                        ProposedEvent::Move {
                            object: *object,
                            destination: Zone::Battlefield,
                        }
                    }
                    EntryOrigin::Cascade { .. } => ProposedEvent::CascadeLandEntry {
                        object: *object,
                        player,
                    },
                    EntryOrigin::Search { .. } => ProposedEvent::SearchLandEntry {
                        object: *object,
                        player,
                    },
                })
                .collect(),
        });
        if matches!(
            origin,
            EntryOrigin::LandPlay { .. } | EntryOrigin::LinkedPlay { .. }
        ) {
            children.push(EventGroup::Occurrence {
                proposed: ProposedEvent::LandPlayed { player },
            });
        }
        Ok(EventGroup::Ordered { children })
    }
    pub(super) fn prepare_entry_batch(
        &self,
        cause: &BatchCause,
        group: &EventGroup,
    ) -> Result<Option<PreparedEntryAuthority>, String> {
        let Frame::LandEntry {
            origin,
            objects,
            choices,
        } = &self.state.rules.frame
        else {
            return Ok(None);
        };
        // Other causes (notably concession/control reconciliation) are ordinary
        // events, even while a chooser is suspended. A cause purporting to be
        // this entry must prove the COMPLETE transaction, including payments.
        let originating = match (origin, cause) {
            (EntryOrigin::LandPlay { .. }, BatchCause::LandPlay { .. }) => true,
            (EntryOrigin::Search { frame }, BatchCause::Instruction { stack_id, .. }) => {
                *stack_id == frame.spell.id
            }
            (
                EntryOrigin::LinkedPlay { progress, .. },
                BatchCause::Instruction { stack_id, .. },
            ) => *stack_id == progress.frame.stack_id,
            (EntryOrigin::Cascade { progress }, BatchCause::Instruction { stack_id, .. }) => {
                *stack_id == progress.frame.stack_id
            }
            _ => false,
        };
        if !originating {
            return Ok(None);
        }
        self.validate_entry_plan()?;
        if *cause != self.entry_cause()? || *group != self.entry_group()? {
            return Err("entry cause/group authority mismatch".into());
        }
        let mut entries = objects
            .iter()
            .map(|o| (*o, None))
            .collect::<BTreeMap<_, _>>();
        for ((object, _, _), paid) in self.entry_offers(objects)?.iter().zip(choices) {
            entries.insert(*object, Some(*paid));
        }
        let (put_by_effect, force_tapped) = match origin {
            EntryOrigin::LandPlay { .. } | EntryOrigin::LinkedPlay { .. } => (false, false),
            EntryOrigin::Cascade { .. } => (true, true),
            EntryOrigin::Search { frame } => (true, self.entry_search_force_tapped(frame)?),
        };
        // No Basic+Land scan here. The single simultaneous capture follows ONLY
        // life losses; no battlefield, control or entry can change its prior facts.
        Ok(Some(PreparedEntryAuthority {
            player: origin.player(),
            put_by_effect,
            force_tapped,
            entries,
        }))
    }
    pub(super) fn land_entry_step(&mut self) -> Result<(), String> {
        self.validate_entry_plan()?;
        if self.land_entry_offer()?.is_some() {
            let input = self
                .state
                .rules
                .pending
                .take()
                .ok_or("missing entry decision")?;
            self.validate_land_entry_response(input.player, &input.response)?;
            let Response::OptionalEffect { perform } = input.response else {
                return Err("entry input invariant".into());
            };
            let Frame::LandEntry { choices, .. } = &mut self.state.rules.frame else {
                unreachable!()
            };
            choices.push(perform);
            return Ok(());
        }
        let (origin, objects, _) = self.entry_frame()?;
        let origin = origin.clone();
        let first = objects[0];
        self.commit_batch(self.entry_cause()?, self.entry_group()?)?;
        match origin {
            EntryOrigin::LandPlay { .. } => {
                self.state.rules.frame = Frame::Priority;
                self.state.rules.passes.clear();
                self.stabilize()
            }
            EntryOrigin::LinkedPlay { progress, .. } => {
                self.state.rules.frame = Frame::PlayHideawayLinked { progress };
                Ok(())
            }
            EntryOrigin::Search { frame } => self.resume_entry_search(frame),
            EntryOrigin::Cascade { progress } => self.resume_cascade_land(progress, Some(first)),
        }
    }
    pub(super) fn land_entry_visible_to(&self, viewer: PlayerId, object: ObjectId) -> bool {
        matches!(&self.state.rules.frame, Frame::LandEntry { origin, objects, .. }
            if origin.player() == viewer && self.is_live(viewer) && objects.contains(&object))
    }
    pub(super) fn validate_land_entry_state(&self) -> Result<(), String> {
        if matches!(self.state.rules.frame, Frame::LandEntry { .. }) {
            return self.validate_entry_plan();
        }
        let retained = spell_cascade_grants::retained_effect_frame(&self.state.rules.frame);
        if matches!(retained, Frame::LandEntry { .. }) {
            let mut validation = self.fork();
            validation.state.rules.frame = retained.clone();
            validation.validate_entry_plan()?;
        }
        Ok(())
    }
    pub(super) fn revalidate_land_entry_departure(&mut self) -> Result<(), String> {
        let Frame::LandEntry {
            origin, objects, ..
        } = self.state.rules.frame.clone()
        else {
            return Ok(());
        };
        if let EntryOrigin::LinkedPlay {
            progress,
            object,
            receipt,
        } = &origin
            && (!self.is_live(origin.player())
                || !self.linked_land_playable(origin.player(), *object)?)
        {
            self.state.rules.pending = None;
            // Entry has not committed. A surviving land returns to its
            // original face-down exile state, preserving all prior lookers.
            if self.current(*object).is_ok_and(|o| o.zone == Zone::Exile) {
                self.restore_hideaway_link(receipt.clone())?;
            }
            self.state.rules.frame = Frame::PlayHideawayLinked {
                progress: progress.clone(),
            };
            return Ok(());
        }
        if !self.is_live(origin.player()) {
            self.state.rules.pending = None;
            return match origin {
                EntryOrigin::LandPlay { .. } => {
                    self.state.rules.frame = Frame::Priority;
                    Ok(())
                }
                EntryOrigin::LinkedPlay { .. } => unreachable!("departed linked land was unwound"),
                EntryOrigin::Cascade { progress } => {
                    self.finish_ability(&progress.frame.spell, false)
                }
                EntryOrigin::Search { frame } => {
                    self.finish_instruction(frame, InstructionOutcome::Effect { actual: None })
                }
            };
        }
        if let EntryOrigin::Cascade { progress } = origin
            && !self.cascade_land_available(&progress, objects[0])?
        {
            self.state.rules.pending = None;
            return self.resume_cascade_land(progress, None);
        }
        // A departed SPELL SOURCE OWNER does not cancel a live controller's
        // frozen paid invocation. Existing resolving validation/LKI owns that case.
        self.validate_entry_plan()?;
        if self.state.rules.pending.as_ref().is_some_and(|p| {
            self.validate_land_entry_response(p.player, &p.response)
                .is_err()
        }) {
            self.state.rules.pending = None;
        }
        Ok(())
    }
    #[cfg(test)]
    pub(super) fn entry_cause_for_test(&self) -> BatchCause {
        self.entry_cause().unwrap()
    }
    #[cfg(test)]
    pub(super) fn entry_group_for_test(&self) -> EventGroup {
        self.entry_group().unwrap()
    }
    pub(super) fn land_entry_charge(&self) -> usize {
        let frame = spell_cascade_grants::retained_effect_frame(&self.state.rules.frame);
        let Frame::LandEntry { origin, .. } = frame else {
            return 0;
        };
        let separately_charged = match origin {
            EntryOrigin::LandPlay { .. }
            | EntryOrigin::Cascade { .. }
            | EntryOrigin::LinkedPlay { .. } => 0,
            EntryOrigin::Search { frame } => costs::entry_cost_lock_charge(&frame.spell)
                .saturating_add(spell_cascade_grants::entry_grant_charge(&frame.spell)),
        };
        // Charge ALL retained frame bytes (including the otherwise uncharged
        // Resolving metadata), less ONLY the exact values already charged by the
        // root cost/grant owners. Keys/envelopes and the root itself remain charged.
        stored_history::payload_charge(frame).saturating_sub(separately_charged)
    }
}
