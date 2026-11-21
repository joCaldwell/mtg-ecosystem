//! CR 305.2–3, 406.3a, 607.2a/3, 608.2g, 702.75a.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LinkedPlayProgress {
    pub(super) frame: Resolving,
    pub(super) source_block: u32,
    hideaway_block: u32,
    // A copied hideaway trigger can link several cards. Each completed spell
    // stays above this ability until the controller has finished all plays.
    children: Vec<(ObjectId, StackId)>,
}

impl LinkedPlayProgress {
    pub(super) fn receipt_matches(&self, receipt: &HideawayLink, object: ObjectId) -> bool {
        receipt.matches(self.frame.spell.source, self.hideaway_block, object)
    }
    pub(super) fn permission(&self, object: ObjectId) -> CastPermission {
        CastPermission::from_instruction(&self.frame, self.source_block, object, None)
    }

    pub(super) fn complete_cast(&mut self, object: ObjectId, child: StackId) {
        self.children.push((object, child));
    }
}

impl Game {
    pub(super) fn begin_linked_play(
        &mut self,
        frame: Resolving,
        hideaway_block: u32,
        minimum_total_power: Quantity,
        source_block: u32,
    ) -> Result<(), String> {
        // Test the condition once, when carrying out the effect, including
        // negative power. Subsequent plays can themselves change the battlefield.
        let mut power = Quantity::zero();
        for object in self.controlled_creatures(frame.spell.controller) {
            power.add(
                &self
                    .characteristics(object)?
                    .power
                    .ok_or("missing creature power")?,
            );
        }
        if power < minimum_total_power {
            return self.finish_ability(&frame.spell, true);
        }
        self.state.rules.frame = Frame::PlayHideawayLinked {
            progress: Box::new(LinkedPlayProgress {
                frame,
                hideaway_block,
                source_block,
                children: Vec::new(),
            }),
        };
        Ok(())
    }

    pub(super) fn linked_land_playable(
        &self,
        player: PlayerId,
        object: ObjectId,
    ) -> Result<bool, String> {
        self.linked_land_face_playable(player, object, false)
    }

    pub(super) fn linked_land_face_playable(
        &self,
        player: PlayerId,
        object: ObjectId,
        back: bool,
    ) -> Result<bool, String> {
        let definition = self.definition(object.card)?;
        let definition = if back {
            let Some(back) = definition.modal_land_back() else {
                return Ok(false);
            };
            back
        } else {
            definition
        };
        Ok(self.state.rules.active == Some(player)
            && self.has_land_play_allowance(player)
            && self.current(object).is_ok_and(|o| o.zone == Zone::Exile)
            && definition.characteristics.has_type(CardKind::Land)
            && definition.capabilities.contains(&Capability::LandPlayV1))
    }

    pub(super) fn linked_front_choice_available(
        &self,
        player: PlayerId,
        object: ObjectId,
    ) -> Result<bool, String> {
        Ok(self.linked_land_playable(player, object)?
            || self.resolution_spell_proposable(player, object)?)
    }

    fn linked_play_candidates(
        &self,
        progress: &LinkedPlayProgress,
    ) -> Result<Vec<ObjectId>, String> {
        let player = progress.frame.spell.controller;
        let mut candidates = Vec::new();
        if self.is_live(player) {
            for object in
                self.linked_hideaway_cards(progress.frame.spell.source, progress.hideaway_block)
            {
                if self.linked_front_choice_available(player, object)?
                    || self.linked_land_face_playable(player, object, true)?
                {
                    candidates.push(object);
                }
            }
        }
        Ok(candidates)
    }

    pub(super) fn linked_play_request(&self) -> Result<Option<Request>, String> {
        let Frame::PlayHideawayLinked { progress } = &self.state.rules.frame else {
            return Ok(None);
        };
        let candidates = self.linked_play_candidates(progress)?;
        Ok(
            (!candidates.is_empty()).then_some(Request::PlayHideawayLinked {
                decision_id: self.state.rules.revision,
                player: progress.frame.spell.controller,
                candidates,
            }),
        )
    }

    pub(super) fn validate_linked_play_response(
        &self,
        actor: PlayerId,
        response: &Response,
    ) -> Result<(), String> {
        if let Some(Request::PlayHideawayLinked {
            player, candidates, ..
        }) = self.linked_play_request()?
            && let Response::PlayHideawayLinked { object } = response
            && actor == player
            && match object {
                None => true,
                Some(id) => {
                    candidates.contains(id) && self.linked_front_choice_available(player, *id)?
                }
            }
        {
            return Ok(());
        }
        if let Response::PlayHideawayLinkedBackLand { object } = response
            && let Some(Request::PlayHideawayLinked {
                player, candidates, ..
            }) = self.linked_play_request()?
            && actor == player
            && candidates.contains(object)
            && self.linked_land_face_playable(player, *object, true)?
        {
            return Ok(());
        }
        Err("unavailable linked card choice".into())
    }

    pub(super) fn linked_play_step(
        &mut self,
        progress: Box<LinkedPlayProgress>,
    ) -> Result<(), String> {
        let player = progress.frame.spell.controller;
        if let Some(input) = self.state.rules.pending.take() {
            self.validate_linked_play_response(input.player, &input.response)?;
            let choice = match input.response {
                Response::PlayHideawayLinked { object } => object.map(|id| (id, false)),
                Response::PlayHideawayLinkedBackLand { object } => Some((object, true)),
                _ => None,
            };
            if let Some((object, back_face)) = choice {
                let receipt = self.turn_up_linked_hideaway(
                    progress.frame.spell.source,
                    progress.hideaway_block,
                    object,
                )?;
                if self.linked_land_face_playable(player, object, back_face)? {
                    self.begin_land_entry(
                        EntryOrigin::LinkedPlay {
                            progress,
                            object,
                            receipt,
                            back_face,
                        },
                        vec![object],
                    )?;
                    return Ok(());
                }
                return self.announce_resolution_spell(CastOwner::LinkedPlay {
                    progress,
                    object,
                    receipt,
                });
            }
        } else if self.linked_play_request()?.is_some() {
            return Err("missing linked play decision".into());
        }
        self.finish_ability(&progress.frame.spell, self.is_live(player))
    }

    pub(super) fn revalidate_linked_play_departure(&mut self) -> Result<(), String> {
        if matches!(self.state.rules.frame, Frame::PlayHideawayLinked { .. })
            && let Some(input) = &self.state.rules.pending
            && self
                .validate_linked_play_response(input.player, &input.response)
                .is_err()
        {
            self.state.rules.pending = None;
        }
        Ok(())
    }

    pub(super) fn linked_play_children(&self, progress: &LinkedPlayProgress) -> Vec<StackId> {
        progress
            .children
            .iter()
            .filter_map(|(object, id)| {
                // A foreign owner leaving removes their spell immediately.
                self.is_live(self.state.rules.cards[&object.card].owner)
                    .then_some(*id)
            })
            .collect()
    }

    pub(super) fn validate_linked_play(
        &self,
        progress: &LinkedPlayProgress,
        announcing: Option<(&CastPermission, ObjectId)>,
    ) -> Result<(), String> {
        let frame = &progress.frame;
        if !matches!(frame.spell.kind, StackKind::Activated { .. })
            || frame.cursor != 0
            || !frame.legal_targets.is_empty()
            || !matches!(self.program_for(&frame.spell)?.instructions.as_slice(),
                [Instruction::PlayHideawayLinked { source_block, hideaway_block, .. }]
                if *source_block == progress.source_block && *hideaway_block == progress.hideaway_block)
            || progress.children.len() > self.state.rules.cards.len()
        {
            return Err("snapshot linked play issuer".into());
        }
        let mut objects = BTreeSet::new();
        let mut children = Vec::new();
        for &(object, id) in &progress.children {
            let card = self
                .state
                .rules
                .cards
                .get(&object.card)
                .ok_or("unknown linked spell")?;
            if !objects.insert(object.card)
                || id <= frame.stack_id
                || id.0 >= self.state.rules.next_stack_id
            {
                return Err("snapshot duplicate or invalid linked child".into());
            }
            if self.is_live(card.owner) && self.is_live(frame.spell.controller) {
                let entry = self
                    .state
                    .rules
                    .stack
                    .iter()
                    .find(|s| s.id == id)
                    .ok_or("missing paid linked child")?;
                if entry.paid.is_none()
                    || !self.resulting_permission_matches(&progress.permission(object), entry)?
                {
                    return Err("snapshot linked paid child".into());
                }
                children.push(id);
            }
        }
        if let Some((permission, object)) = announcing {
            if !objects.insert(object.card) || *permission != progress.permission(object) {
                return Err("snapshot linked casting permission".into());
            }
            children.push(self.top()?.id);
        }
        self.validate_resolving_children(frame, &children)
    }
}
