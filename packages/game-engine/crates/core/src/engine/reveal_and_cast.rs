//! CR 401, 608.2c/g, 701 (reveal and shuffle), 118.9, 601.2.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "stage", rename_all = "snake_case", deny_unknown_fields)]
enum RevealStage {
    Scan,
    Exile {
        winner: ObjectId,
    },
    RandomBottom {
        hit: Option<ObjectId>,
        order: Vec<ObjectId>,
        index: u32,
    },
    Offer {
        hit: ObjectId,
    },
    Cast {
        hit: ObjectId,
        child: StackId,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RevealAndCastProgress {
    pub(super) frame: Resolving,
    pub(super) source_block: u32,
    lands: Vec<ObjectId>,
    stage: RevealStage,
}

impl RevealAndCastProgress {
    pub(super) fn after_shuffle(frame: Resolving, source_block: u32) -> Self {
        Self {
            frame,
            source_block,
            lands: vec![],
            stage: RevealStage::Scan,
        }
    }
    pub(super) fn permission(&self) -> Result<CastPermission, String> {
        let RevealStage::Offer { hit } = self.stage else {
            return Err("no revealed card cast offer".into());
        };
        Ok(CastPermission::from_instruction(
            &self.frame,
            self.source_block,
            hit,
            None,
        ))
    }
    pub(super) fn child(&self) -> Option<StackId> {
        match self.stage {
            RevealStage::Cast { child, .. } => Some(child),
            _ => None,
        }
    }
    pub(super) fn complete_cast(&mut self, hit: ObjectId, child: StackId) -> Result<(), String> {
        if self.permission()?.object != hit {
            return Err("revealed cast completion identity".into());
        }
        self.stage = RevealStage::Cast { hit, child };
        Ok(())
    }
    fn random_bottom(&self, hit: Option<ObjectId>) -> Result<RevealStage, String> {
        if self.lands.len() > capacities::OWNED_LIMIT {
            return Err("revealed library bound".into());
        }
        Ok(RevealStage::RandomBottom {
            hit,
            order: self.lands.clone(),
            index: self.lands.len().saturating_sub(1) as u32,
        })
    }
}

impl Game {
    pub(super) fn resolution_cast_request(&self) -> Result<Option<Request>, String> {
        let Frame::RevealAndCast { progress } = &self.state.rules.frame else {
            return self.cascade_request();
        };
        let player = progress.frame.spell.controller;
        if !self.is_live(player) || !matches!(progress.stage, RevealStage::Offer { .. }) {
            return Ok(None);
        }
        Ok(self
            .resolution_offer_legal(&progress.permission()?)?
            .then_some(Request::OptionalEffect {
                decision_id: self.state.rules.revision,
                player,
            }))
    }

    pub(super) fn validate_reveal_offer(
        &self,
        progress: &RevealAndCastProgress,
        player: PlayerId,
        response: &Response,
    ) -> Result<(), String> {
        if player != progress.frame.spell.controller
            || !matches!(response, Response::OptionalEffect { .. })
            || !self.is_live(player)
            || !self.resolution_offer_legal(&progress.permission()?)?
        {
            return Err("invalid revealed card cast choice".into());
        }
        Ok(())
    }
    pub(super) fn revalidate_reveal_departure(&mut self) -> Result<(), String> {
        if let Frame::RevealAndCast { progress } = &self.state.rules.frame
            && let Some(input) = &self.state.rules.pending
        {
            if input.player != progress.frame.spell.controller
                || !matches!(input.response, Response::OptionalEffect { .. })
            {
                return Err("unexpected pending revealed cast input".into());
            }
            if !self.is_live(input.player)
                || !self.resolution_offer_legal(&progress.permission()?)?
            {
                self.state.rules.pending = None;
            }
        }
        Ok(())
    }

    fn next_unrevealed(&self, player: PlayerId, index: usize) -> Result<Option<ObjectId>, String> {
        self.state
            .rules
            .libraries
            .get(&player)
            .and_then(|lib| lib.get(index))
            .map(|card| {
                self.object_id(*card)
                    .ok_or_else(|| "missing reveal library incarnation".into())
            })
            .transpose()
    }

    pub(super) fn reveal_and_cast_step(
        &mut self,
        mut progress: Box<RevealAndCastProgress>,
    ) -> Result<(), String> {
        let player = progress.frame.spell.controller;
        let cause = Self::wipe_cause(&progress.frame, progress.source_block);
        if !self.is_live(player) {
            return self.finish_spell(&progress.frame.spell, false);
        }
        match &mut progress.stage {
            RevealStage::Scan => {
                if let Some(object) = self.next_unrevealed(player, progress.lands.len())? {
                    self.commit(ProposedEvent::RevealLibraryCard { player, object }, cause)?;
                    if self
                        .definition(object.card)?
                        .characteristics
                        .has_type(CardKind::Land)
                    {
                        progress.lands.push(object);
                    } else {
                        progress.stage = RevealStage::Exile { winner: object };
                    }
                } else {
                    progress.stage = progress.random_bottom(None)?;
                }
            }
            RevealStage::Exile { winner } => {
                let hit = self.move_object_for(*winner, Zone::Exile, cause)?.successor;
                progress.stage = progress.random_bottom(Some(hit))?;
            }
            RevealStage::RandomBottom { hit, order, index } => {
                if *index > 0 {
                    // Private Fisher-Yates. Rejection consumes one word and one
                    // budget step without revealing an intermediate permutation.
                    let rng = &mut self
                        .state
                        .rules
                        .match_state
                        .as_mut()
                        .ok_or("missing trusted random bottom stream")?
                        .random;
                    if let Some(j) = setup::sample_shuffle_word(rng.word()?, *index + 1) {
                        order.swap(*index as usize, j as usize);
                        *index -= 1;
                    }
                } else {
                    if !order.is_empty() {
                        self.commit(
                            ProposedEvent::BottomRevealedCards {
                                player,
                                order: order.clone(),
                            },
                            cause,
                        )?;
                    }
                    progress.lands.clear();
                    let Some(hit) = *hit else {
                        return self.finish_spell(&progress.frame.spell, true);
                    };
                    progress.stage = RevealStage::Offer { hit };
                }
            }
            RevealStage::Offer { .. } => {
                let legal = self.resolution_offer_legal(&progress.permission()?)?;
                if let Some(input) = self.state.rules.pending.take() {
                    let Response::OptionalEffect { perform } = input.response else {
                        return Err("revealed cast response shape".into());
                    };
                    if input.player != player {
                        return Err("revealed cast actor".into());
                    }
                    if perform && legal {
                        return self.announce_resolution_spell(CastOwner::RevealAndCast(progress));
                    }
                } else if legal {
                    return Err("missing revealed cast decision".into());
                }
                return self.finish_spell(&progress.frame.spell, true);
            }
            RevealStage::Cast { .. } => return self.finish_spell(&progress.frame.spell, true),
        }
        self.state.rules.frame = Frame::RevealAndCast { progress };
        Ok(())
    }

    pub(super) fn apply_reveal_library_card(
        &mut self,
        player: PlayerId,
        object: ObjectId,
    ) -> Result<EventOutcome, String> {
        if self.look_top_mana_reveal_authorized(player, object) {
            self.remember_public_library_position(object);
            return Ok(EventOutcome::LibraryCardRevealed { player, object });
        }
        let Frame::RevealAndCast { progress } = &self.state.rules.frame else {
            return Err("missing reveal owner".into());
        };
        if !matches!(progress.stage, RevealStage::Scan)
            || progress.frame.spell.controller != player
            || self.next_unrevealed(player, progress.lands.len())? != Some(object)
            || self.current(object)?.zone != (Zone::Library { owner: player })
        {
            return Err("reveal outside actual library scan".into());
        }
        self.remember_public_library_position(object);
        Ok(EventOutcome::LibraryCardRevealed { player, object })
    }
    pub(super) fn apply_bottom_revealed(
        &mut self,
        player: PlayerId,
        order: &[ObjectId],
    ) -> Result<EventOutcome, String> {
        let Frame::RevealAndCast { progress } = &self.state.rules.frame else {
            return Err("missing random bottom owner".into());
        };
        if progress.frame.spell.controller != player
            || !matches!(&progress.stage, RevealStage::RandomBottom { index: 0, order: sampled, .. } if sampled == order)
            || self.library_prefix(player, &Quantity::from(order.len() as i64))? != progress.lands
        {
            return Err("random bottom prefix or sampled order invariant".into());
        }
        let prefix = progress.lands.clone();
        self.reorder_library_prefix(player, &prefix, &[], order)?;
        Ok(EventOutcome::RevealedCardsBottomed {
            player,
            order: order.to_vec(),
        })
    }

    pub(super) fn validate_reveal_and_cast(
        &self,
        progress: &RevealAndCastProgress,
        announcing: Option<&CastPermission>,
    ) -> Result<(), String> {
        let frame = &progress.frame;
        let player = frame.spell.controller;
        let child = if announcing.is_some() {
            Some(self.top()?.id)
        } else {
            progress.child()
        };
        self.validate_resolving_owned(frame, child)?;
        if !frame.spell.kind.is_spell()
            || frame.cursor != 0
            || !matches!(self.program_for(&frame.spell)?.instructions.as_slice(), [Instruction::RevealAndCast { source_block }] if *source_block == progress.source_block)
            || announcing.is_some_and(|p| progress.permission().as_ref() != Ok(p))
            || progress.lands.len() > capacities::OWNED_LIMIT
        {
            return Err("snapshot reveal instruction ownership".into());
        }
        // Owner departure removes its library and stack objects in one batch.
        // Replay proves the historical scan up to that boundary.
        if !self.is_live(player) {
            return Ok(());
        }
        if self.library_prefix(player, &Quantity::from(progress.lands.len() as i64))?
            != progress.lands
            || progress.lands.iter().any(|o| {
                !self
                    .definition(o.card)
                    .is_ok_and(|d| d.characteristics.has_type(CardKind::Land))
            })
        {
            return Err("snapshot reveal land prefix".into());
        }
        let hit = match &progress.stage {
            RevealStage::Scan => None,
            RevealStage::Exile { winner } => {
                if self.next_unrevealed(player, progress.lands.len())? != Some(*winner)
                    || self
                        .definition(winner.card)?
                        .characteristics
                        .has_type(CardKind::Land)
                {
                    return Err("snapshot first revealed nonland".into());
                }
                None
            }
            RevealStage::RandomBottom { hit, order, index } => {
                self.validate_library_partition(player, &progress.lands, &[], order)?;
                if *index as usize >= order.len().max(1)
                    || hit.is_none() && self.next_unrevealed(player, order.len())?.is_some()
                {
                    return Err("snapshot reveal random bottom cursor".into());
                }
                *hit
            }
            RevealStage::Offer { hit } | RevealStage::Cast { hit, .. } => {
                if !progress.lands.is_empty() {
                    return Err("snapshot unfinished reveal bottom".into());
                }
                Some(*hit)
            }
        };
        if let Some(hit) = hit {
            if self
                .state
                .rules
                .cards
                .get(&hit.card)
                .is_none_or(|c| c.owner != player)
                || self
                    .definition(hit.card)?
                    .characteristics
                    .has_type(CardKind::Land)
            {
                return Err("snapshot revealed exile winner".into());
            }
            if child.is_some() {
                let top = self.top()?;
                let permission =
                    CastPermission::from_instruction(frame, progress.source_block, hit, None);
                if !self.resulting_permission_matches(&permission, top)?
                    || (announcing.is_none() && top.paid.is_none())
                {
                    return Err("snapshot revealed child cast identity".into());
                }
            } else if self.current(hit)?.zone != Zone::Exile {
                return Err("snapshot missing revealed exile incarnation".into());
            }
        }
        Ok(())
    }
}
