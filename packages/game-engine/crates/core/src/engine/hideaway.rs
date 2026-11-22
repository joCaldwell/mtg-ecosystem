//! CR 400.7, 406.3–4, 607.2a, 608.2h and 702.75a–b.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

/// Stored on the exact exile object. Moving or removing that object discards
/// the link, independently of what happens to its source or either controller.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HideawayLink {
    card: ObjectId,
    source: ObjectId,
    source_block: u32,
    source_handles: [publish::StoredObjectHandle; 4],
    lookers: [bool; 4],
}

impl HideawayLink {
    pub fn matches(&self, source: ObjectId, source_block: u32, card: ObjectId) -> bool {
        self.source == source && self.source_block == source_block && self.card == card
    }

    pub(super) fn source_handle(&self, viewer: PlayerId) -> &publish::StoredObjectHandle {
        &self.source_handles[viewer.0 as usize]
    }

    pub(super) fn may_look(&self, viewer: PlayerId) -> bool {
        self.lookers[viewer.0 as usize]
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RandomBottom {
    selected: ObjectId,
    order: Vec<ObjectId>,
    remaining: u32,
}

impl RulesTxn {
    pub(crate) fn linked_hideaway_cards(
        &self,
        source: ObjectId,
        source_block: u32,
    ) -> Vec<ObjectId> {
        self.state
            .rules
            .objects
            .values()
            .filter(|o| {
                o.zone == Zone::Exile
                    && o.hideaway
                        .as_ref()
                        .is_some_and(|link| link.matches(source, source_block, o.id))
            })
            .map(|o| o.id)
            .collect()
    }

    pub(crate) fn turn_up_linked_hideaway(
        &mut self,
        source: ObjectId,
        source_block: u32,
        card: ObjectId,
    ) -> Result<HideawayLink, String> {
        let o = self
            .rules_mut()
            .objects
            .get_mut(&card.card)
            .filter(|o| o.id == card && o.zone == Zone::Exile)
            .ok_or("missing linked exile incarnation")?;
        let link = o
            .hideaway
            .take_if(|link| link.matches(source, source_block, card))
            .ok_or("card was not hidden by this ability")?;
        Ok(*link)
    }

    pub(crate) fn restore_hideaway_link(
        &mut self,
        mut receipt: HideawayLink,
    ) -> Result<(), String> {
        if let Ok(source) = self.current(receipt.source)
            && source.present_on_battlefield()
            && self.is_live(source.controller)
        {
            receipt.lookers[source.controller.0 as usize] = true;
        }
        // A reversed cast may have been attempted by a player other than the owner.
        let owner = self.state.rules.cards[&receipt.card.card].owner;
        let o = self
            .rules_mut()
            .objects
            .get_mut(&receipt.card.card)
            .filter(|o| o.id == receipt.card && o.zone == Zone::Exile && o.hideaway.is_none())
            .ok_or("reversal did not restore exact unlinked exile incarnation")?;
        o.controller = owner;
        o.default_controller = owner;
        o.hideaway = Some(Box::new(receipt));
        Ok(())
    }

    pub(super) fn grant_hideaway_looks(&mut self, controllers: &BTreeMap<ObjectId, PlayerId>) {
        let live = std::array::from_fn::<_, 4, _>(|seat| self.is_live(PlayerId(seat as u8)));
        for o in self.rules_mut().objects.values_mut() {
            if let Some(link) = &mut o.hideaway
                && let Some(p) = controllers.get(&link.source)
                && live[p.0 as usize]
            {
                link.lookers[p.0 as usize] = true;
            }
        }
    }

    pub(super) fn validate_hideaway_links(&self) -> Result<(), String> {
        for o in self.state.rules.objects.values() {
            let Some(link) = &o.hideaway else { continue };
            let owner = self
                .state
                .rules
                .cards
                .get(&o.id.card)
                .ok_or("missing hideaway card owner")?
                .owner;
            let producer = self.copiable_definition(link.source)?.triggered.iter().any(|ability| {
                ability.condition == TriggerCondition::SelfEntersBattlefield
                    && matches!(ability.program.instructions.as_slice(),
                        [Instruction::Hideaway { source_block, .. }] if *source_block == link.source_block)
            });
            let mut controller_permitted = true;
            if let Some(current) = self.state.rules.objects.get(&link.source.card) {
                if link.source.incarnation > current.id.incarnation
                    || (current.id == link.source && current.zone != Zone::Battlefield)
                {
                    return Err("invalid hideaway source incarnation".into());
                }
                if current.id == link.source
                    && current.present_on_battlefield()
                    && self.is_live(current.controller)
                {
                    controller_permitted = link.may_look(current.controller);
                }
            }
            if o.id != link.card
                || o.zone != Zone::Exile
                || o.controller != owner
                || !link.may_look(owner)
                || !producer
                || !controller_permitted
            {
                return Err("invalid face-down exile link".into());
            }
        }
        Ok(())
    }

    fn hideaway_spec(&self, frame: &Resolving) -> Result<(u32, u32), String> {
        let StackKind::Triggered {
            ability: EffectiveTriggerAbility::Intrinsic { index },
            witness,
            grant: None,
            ..
        } = &self.entry(frame.stack_id)?.kind
        else {
            return Err("hideaway requires its intrinsic entry trigger".into());
        };
        let source = self.entry(frame.stack_id)?.source;
        let ability = self
            .copiable_definition(source)?
            .triggered
            .get(*index as usize)
            .ok_or("missing hideaway trigger")?;
        let [
            Instruction::Hideaway {
                count,
                source_block,
            },
        ] = ability.program.instructions.as_slice()
        else {
            return Err("invalid hideaway program".into());
        };
        let entered = match witness {
            TriggerEventWitness::Moved { movement } => {
                movement.destination == Zone::Battlefield && movement.successor == source
            }
            TriggerEventWitness::TokenEntered { object } => *object == source,
            _ => false,
        };
        if !entered
            || frame.cursor != 0
            || ability.condition != TriggerCondition::SelfEntersBattlefield
            || *count == 0
        {
            return Err("hideaway requires exact self entry".into());
        }
        Ok((*count, *source_block))
    }

    pub(super) fn begin_hideaway(&mut self, frame: Resolving) -> Result<(), String> {
        let player = self.entry(frame.stack_id)?.controller;
        if !self.is_live(player) {
            return self
                .finish_instruction(frame, InstructionOutcome::PlayerActionSkipped { player });
        }
        let (count, _) = self.hideaway_spec(&frame)?;
        let looked = self.library_prefix(player, &Quantity::from(i64::from(count)))?;
        if looked.is_empty() {
            return self.finish_instruction(frame, InstructionOutcome::Effect { actual: None });
        }
        self.rules_mut().frame = Frame::ResolutionChoice {
            frame,
            choice: ResolutionChoice::Hideaway {
                player,
                looked,
                bottom: None,
            },
        };
        Ok(())
    }

    pub(super) fn validate_hideaway_choice(
        &self,
        frame: &Resolving,
        player: PlayerId,
        looked: &[ObjectId],
        bottom: Option<&RandomBottom>,
    ) -> Result<(), String> {
        let (count, _) = self.hideaway_spec(frame)?;
        if player != self.entry(frame.stack_id)?.controller
            || looked.is_empty()
            || looked.len() > count as usize
            || looked.len() > self.state.rules.cards.len()
            || looked.iter().collect::<BTreeSet<_>>().len() != looked.len()
            || looked.iter().any(|id| {
                self.state
                    .rules
                    .cards
                    .get(&id.card)
                    .is_none_or(|card| card.owner != player)
            })
            || (self.is_live(player)
                && self.library_prefix(player, &Quantity::from(i64::from(count)))? != looked)
        {
            return Err("invalid hideaway prefix".into());
        }
        if let Some(bottom) = bottom
            && (bottom.order.len() + 1 != looked.len()
                || bottom.remaining as usize > bottom.order.len()
                || bottom
                    .order
                    .iter()
                    .chain([&bottom.selected])
                    .collect::<BTreeSet<_>>()
                    != looked.iter().collect::<BTreeSet<_>>())
        {
            return Err("invalid hideaway random-bottom state".into());
        }
        Ok(())
    }

    pub(super) fn hideaway_step(
        &mut self,
        frame: Resolving,
        player: PlayerId,
        looked: Vec<ObjectId>,
        bottom: Option<RandomBottom>,
    ) -> Result<(), String> {
        self.validate_hideaway_choice(&frame, player, &looked, bottom.as_ref())?;
        if !self.is_live(player) {
            self.rules_mut().pending = None;
            return self
                .finish_instruction(frame, InstructionOutcome::PlayerActionSkipped { player });
        }
        let next = match bottom {
            None => {
                let input = self
                    .rules_mut()
                    .pending
                    .take()
                    .ok_or("missing hideaway choice")?;
                let Response::Hideaway { object } = input.response else {
                    return Err("wrong hideaway response".into());
                };
                if input.player != player || !looked.contains(&object) {
                    return Err("invalid hideaway choice".into());
                }
                let order: Vec<_> = looked.iter().copied().filter(|id| *id != object).collect();
                let remaining =
                    u32::try_from(order.len()).map_err(|_| "hideaway length overflow")?;
                RandomBottom {
                    selected: object,
                    order,
                    remaining,
                }
            }
            Some(mut bottom) if bottom.remaining > 1 => {
                // One rejection-sampled Fisher–Yates trial per rules work unit.
                let random = &mut self
                    .rules_mut()
                    .match_state
                    .as_mut()
                    .ok_or("random bottom requires a seeded match")?
                    .random;
                if let Some(index) = setup::sample_bounded_word(random.word()?, bottom.remaining) {
                    bottom.remaining -= 1;
                    bottom.order.swap(bottom.remaining as usize, index as usize);
                }
                bottom
            }
            Some(bottom) => {
                let (_, block) = self.hideaway_spec(&frame)?;
                let actual = self.commit(
                    ProposedEvent::Hideaway {
                        player,
                        looked,
                        selected: bottom.selected,
                        bottom: bottom.order,
                    },
                    self.instruction_cause(&frame, block)?,
                )?;
                return self.finish_instruction(
                    frame,
                    InstructionOutcome::Effect {
                        actual: Some(actual),
                    },
                );
            }
        };
        self.rules_mut().frame = Frame::ResolutionChoice {
            frame,
            choice: ResolutionChoice::Hideaway {
                player,
                looked,
                bottom: Some(next),
            },
        };
        Ok(())
    }

    pub(super) fn authorize_hideaway_event(
        &self,
        cause: &BatchCause,
        proposed: &ProposedEvent,
    ) -> Result<(), String> {
        let Frame::ResolutionChoice {
            frame,
            choice:
                ResolutionChoice::Hideaway {
                    player,
                    looked,
                    bottom: Some(bottom),
                },
        } = &self.state.rules.frame
        else {
            return Err("missing hideaway selection and permutation".into());
        };
        self.validate_resolving(frame)?;
        self.validate_hideaway_choice(frame, *player, looked, Some(bottom))?;
        let (_, block) = self.hideaway_spec(frame)?;
        if !self.is_live(*player)
            || bottom.remaining > 1
            || *cause != self.instruction_cause(frame, block)?
            || !matches!(proposed, ProposedEvent::Hideaway { player:p,looked:l,selected,bottom:b } if p == player && l == looked && *selected == bottom.selected && *b == bottom.order)
        {
            return Err("unbound hideaway exile or random bottom".into());
        }
        Ok(())
    }

    pub(super) fn apply_hideaway(
        &mut self,
        player: PlayerId,
        selected: ObjectId,
        bottom: &[ObjectId],
        before: Option<&PreEventFacts>,
    ) -> Result<EventOutcome, String> {
        let Frame::ResolutionChoice { frame, .. } = &self.state.rules.frame else {
            return Err("missing hideaway frame".into());
        };
        let (_, source_block) = self.hideaway_spec(frame)?;
        let source = self.entry(frame.stack_id)?.source;
        let StackKind::Triggered { source_handles, .. } = &self.entry(frame.stack_id)?.kind else {
            return Err("missing hideaway trigger handles".into());
        };
        let source_handles = source_handles
            .clone()
            .try_into()
            .map_err(|_| "missing hideaway source handle")?;
        let EventOutcome::Moved { movement } = self.apply_occurrence_with_prior(
            &ProposedEvent::Move {
                object: selected,
                destination: Zone::Exile,
            },
            before,
        )?
        else {
            return Err("hideaway did not produce exile successor".into());
        };
        let mut link = HideawayLink {
            card: movement.successor,
            source,
            source_block,
            source_handles,
            lookers: [false; 4],
        };
        link.lookers[player.0 as usize] = true;
        self.restore_hideaway_link(link)?;
        // Movement removed just the selected card, leaving the remaining prefix
        // ahead of the untouched tail. No other card changes zone or incarnation.
        let library = self
            .rules_mut()
            .libraries
            .get_mut(&player)
            .ok_or("missing hideaway library")?;
        let tail_len = library
            .len()
            .checked_sub(bottom.len())
            .ok_or("missing hideaway remainder")?;
        library.rotate_left(bottom.len());
        for (slot, object) in library[tail_len..].iter_mut().zip(bottom) {
            *slot = object.card;
        }
        self.remember_library_action(player, OwnKnowledgeAction::HiddenAway)?;
        Ok(EventOutcome::HiddenAway {
            movement,
            bottom: bottom.to_vec(),
        })
    }
}
