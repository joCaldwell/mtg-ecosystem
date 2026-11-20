//! Finite staples. Pinned CR 115.1, 400.7, 603.5, 603.10a, 608.2b,
//! 701.8a-b, 701.23a-b/e, 701.24a-b and 903.9a-b:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VisibleSearchLand {
    pub handle: ObjectHandle,
    pub oracle_id: String,
}
// Derived authority only; never serialized alongside a resolution choice.
struct LandSearch<'a> {
    typed: Option<&'a TypedLandSearch>,
    maximum: u32,
    split: bool,
    source_block: u32,
}
impl<'a> LandSearch<'a> {
    fn instruction(instruction: &'a Instruction) -> Option<Self> {
        let (typed, maximum, split, source_block) = match instruction {
            Instruction::SearchBasicLands {
                maximum,
                source_block,
            } => (None, *maximum, false, *source_block),
            Instruction::SearchBasicLandsSplit { source_block } => (None, 2, true, *source_block),
            Instruction::SearchTypedLands {
                search,
                source_block,
            } => {
                // Only the offered count is physically bounded. The exact IR
                // quantity remains unchanged, including arbitrarily large maxima.
                let bound = capacities::OWNED_LIMIT as u32;
                let maximum = if search.maximum >= Quantity::from(i64::from(bound)) {
                    bound
                } else {
                    search.maximum.to_string().parse().ok()?
                };
                (Some(search), maximum, false, *source_block)
            }
            _ => return None,
        };
        Some(Self {
            typed,
            maximum,
            split,
            source_block,
        })
    }
    fn accepts(&self, c: &Characteristics) -> bool {
        self.typed
            .map_or_else(|| c.kind == CardKind::Land && c.basic, |s| s.accepts(c))
    }
    fn force_tapped(&self) -> bool {
        self.typed.is_none_or(|s| s.force_tapped)
    }
}
impl Game {
    fn land_search_for<'a>(&'a self, frame: &'a Resolving) -> Option<LandSearch<'a>> {
        LandSearch::instruction(
            self.program_for(&frame.spell)
                .ok()?
                .instructions
                .get(frame.cursor as usize)?,
        )
    }
    fn active_land_search(&self) -> Option<LandSearch<'_>> {
        if let Frame::LandEntry {
            origin: EntryOrigin::Search { frame },
            ..
        } = &self.state.rules.frame
        {
            return self.land_search_for(frame);
        }
        let Frame::ResolutionChoice { frame, choice } = &self.state.rules.frame else {
            return None;
        };
        if !matches!(
            choice,
            ResolutionChoice::Search { .. }
                | ResolutionChoice::SearchSplit { .. }
                | ResolutionChoice::Shuffle { .. }
        ) {
            return None;
        }
        self.land_search_for(frame)
    }
    pub(super) fn searched_land_force_tapped(&self, player: PlayerId) -> Result<bool, String> {
        if let Frame::LandEntry {
            origin: EntryOrigin::Search { frame },
            ..
        } = &self.state.rules.frame
        {
            if frame.spell.controller != player {
                return Err("wrong entry search controller".into());
            }
            return self.entry_search_force_tapped(frame);
        }
        let Frame::ResolutionChoice { frame, choice } = &self.state.rules.frame else {
            return Err("no searched land entry".into());
        };
        if frame.spell.controller != player
            || !matches!(
                choice,
                ResolutionChoice::Search { .. } | ResolutionChoice::SearchSplit { .. }
            )
        {
            return Err("wrong searched land entry controller or phase".into());
        }
        Ok(self
            .land_search_for(frame)
            .ok_or("not land search instruction")?
            .force_tapped())
    }
    pub(super) fn entry_search_block(&self, frame: &Resolving) -> Result<u32, String> {
        let search = self
            .land_search_for(frame)
            .ok_or("not an entry search instruction")?;
        if search.split {
            return Err("split search is not optional entry producer".into());
        }
        Ok(search.source_block)
    }
    pub(super) fn entry_search_force_tapped(&self, frame: &Resolving) -> Result<bool, String> {
        self.entry_search_block(frame)?;
        Ok(self
            .land_search_for(frame)
            .ok_or("missing entry search")?
            .force_tapped())
    }
    pub(super) fn validate_entry_search(
        &self,
        frame: &Resolving,
        objects: &[ObjectId],
    ) -> Result<(), String> {
        self.entry_search_block(frame)?;
        let search = self.land_search_for(frame).ok_or("missing entry search")?;
        let player = frame.spell.controller;
        if !self.is_live(player)
            || objects.len() > search.maximum as usize
            || objects.iter().any(|o| {
                self.current(*o).is_err()
                    || self.zone(*o) != Some(Zone::Library { owner: player })
                    || !self
                        .definition(o.card)
                        .is_ok_and(|d| search.accepts(&d.characteristics))
            })
        {
            return Err("invalid complete entry search selection".into());
        }
        Ok(())
    }
    pub(super) fn resume_entry_search(&mut self, frame: Resolving) -> Result<(), String> {
        let source_block = self.entry_search_block(&frame)?;
        self.begin_library_shuffle(frame, source_block)
    }
    pub(super) fn begin_library_shuffle(
        &mut self,
        frame: Resolving,
        source_block: u32,
    ) -> Result<(), String> {
        let player = frame.spell.controller;
        self.begin_player_library_shuffle(frame, player, source_block)
    }
    pub(super) fn resolution_draw_player(&self, frame: &Resolving) -> Result<PlayerId, String> {
        let instruction = self
            .program_for(&frame.spell)?
            .instructions
            .get(frame.cursor as usize)
            .ok_or("missing draw instruction")?;
        Self::draw_player_for_instruction(frame, instruction)
    }

    pub(super) fn draw_player_for_instruction(
        frame: &Resolving,
        instruction: &Instruction,
    ) -> Result<PlayerId, String> {
        match instruction {
            Instruction::DrawEachPlayer { .. } => frame
                .each_player_draw
                .as_ref()
                .and_then(|scope| scope.remaining.first().copied())
                .ok_or_else(|| "missing current each-player draw seat".into()),
            Instruction::DrawCards {
                recipient: Some(LifeRecipient::Target { target_slot }),
                ..
            }
            | Instruction::DrawAnnouncedX {
                recipient: LifeRecipient::Target { target_slot },
                ..
            } => frame
                .spell
                .player_targets
                .get(*target_slot as usize)
                .copied()
                .ok_or_else(|| "missing draw target".into()),
            Instruction::DrawCards {
                recipient: Some(LifeRecipient::EventPlayer),
                ..
            } => frame
                .spell
                .kind
                .event_player()?
                .ok_or_else(|| "absent draw event player".into()),
            Instruction::DrawCards {
                recipient: Some(LifeRecipient::EachOpponent),
                ..
            } => Err("unsupported each-opponent draw".into()),
            Instruction::DrawCards {
                recipient: None | Some(LifeRecipient::Controller),
                ..
            }
            | Instruction::DrawAnnouncedX {
                recipient: LifeRecipient::Controller,
                ..
            }
            | Instruction::OptionalDrawUnlessCasterPays { .. }
            | Instruction::OptionalDrawCards { .. }
            | Instruction::DrawControllerCards { .. }
            | Instruction::DrawCardsIfAdditionalCost { .. }
            | Instruction::DrawDamageAmount { .. } => Ok(frame.spell.controller),
            Instruction::OptionalDrawForSoleGreatestEntrant { .. } => frame
                .entrant_draw
                .as_ref()
                .map(|entrant| entrant.controller)
                .ok_or_else(|| "missing resolved entrant".into()),
            _ => Err("not draw instruction".into()),
        }
    }

    pub(super) fn graveyard_target_legal(
        &self,
        object: &Object,
        player: PlayerId,
        domain: TargetDomain,
    ) -> bool {
        self.player_graveyard_card(object, player)
            && match domain {
                TargetDomain::OwnGraveyardCard => true,
                TargetDomain::OwnGraveyardCreatureCard => self
                    .definition(object.id.card)
                    .is_ok_and(|d| d.characteristics.kind == CardKind::Creature),
                _ => false,
            }
    }
    pub(super) fn creature_or_planeswalker(&self, id: ObjectId) -> bool {
        self.current(id).is_ok_and(|o| o.present_on_battlefield())
            && self
                .characteristics(id)
                .is_ok_and(|c| c.has_type(CardKind::Creature))
    }
    pub(super) fn object_target_legal(&self, entry: &StackInstance, object: ObjectId) -> bool {
        let announced = matches!(self.state.rules.frame, Frame::Targets { .. })
            && self.top().is_ok_and(|top| top.id == entry.id);
        let program = if announced {
            self.program()
        } else {
            self.program_for(entry)
        };
        let spec = program.ok().and_then(|p| p.targets.first());
        self.object_target_legal_for(entry, object, spec)
    }
    pub(super) fn object_target_legal_for(
        &self,
        entry: &StackInstance,
        object: ObjectId,
        spec: Option<&mtg_contracts::TargetSpec>,
    ) -> bool {
        if !self
            .targeting_characteristics(entry)
            .is_ok_and(|source| !self.protected_from(object, &source))
        {
            return false;
        }
        match spec.map(|t| t.domain) {
            Some(
                TargetDomain::BattlefieldPermanent | TargetDomain::ControlledBattlefieldPermanent,
            ) => spec.is_some_and(|t| self.permanent_target_legal(object, entry.controller, t)),
            Some(
                domain @ (TargetDomain::OwnGraveyardCreatureCard | TargetDomain::OwnGraveyardCard),
            ) => self
                .current(object)
                .is_ok_and(|o| self.graveyard_target_legal(o, entry.controller, domain)),
            Some(TargetDomain::CreatureOrPlaneswalker) => {
                self.creature_or_planeswalker(object)
                    && self.targetable_by(object, entry.controller)
            }
            Some(TargetDomain::ControlledBattlefieldCreature) => {
                self.is_battlefield_creature(object)
                    && self.targetable_by(object, entry.controller)
                    && self
                        .current(object)
                        .is_ok_and(|o| o.controller == entry.controller && o.id != entry.source)
            }
            Some(TargetDomain::BlockingBattlefieldCreature) => {
                self.is_blocking_creature(object) && self.targetable_by(object, entry.controller)
            }
            _ => {
                self.is_battlefield_creature(object) && self.targetable_by(object, entry.controller)
            }
        }
    }
    pub(super) fn search_lands(&self, player: PlayerId) -> Vec<ObjectId> {
        let descriptor = self.active_land_search();
        // CardId order is physical order, never library order (CR 401.2).
        let mut cards = self
            .state
            .rules
            .objects
            .values()
            .filter(|o| {
                o.zone == (Zone::Library { owner: player })
                    && self.definition(o.id.card).is_ok_and(|d| {
                        descriptor.as_ref().map_or_else(
                            || d.characteristics.kind == CardKind::Land && d.characteristics.basic,
                            |s| s.accepts(&d.characteristics),
                        )
                    })
            })
            .map(|o| o.id)
            .collect::<Vec<_>>();
        cards.sort_by_key(|o| (self.definition(o.card).unwrap().oracle_id.clone(), o.card));
        cards
    }
    pub(super) fn search_handle(
        &self,
        player: PlayerId,
        object: ObjectId,
    ) -> Result<ObjectHandle, String> {
        if !matches!(self.state.rules.frame, Frame::ResolutionChoice { choice: ResolutionChoice::Search { player: p, .. }, .. } if p == player)
        {
            return Err("no private land search".into());
        }
        let index = self
            .search_lands(player)
            .iter()
            .position(|o| *o == object)
            .ok_or("not a search candidate")?;
        let token = self.search_decision_token(player)?;
        Ok(ObjectHandle(format!("{}:search:{index}", token.0)))
    }
    pub(super) fn resolve_search_handle(
        &self,
        player: PlayerId,
        handle: &ObjectHandle,
    ) -> Result<ObjectId, String> {
        let prefix = format!("{}:search:", self.search_decision_token(player)?.0);
        let index = handle
            .0
            .strip_prefix(&prefix)
            .and_then(|n| n.parse::<usize>().ok())
            .ok_or("stale or foreign search handle")?;
        let object = self
            .search_lands(player)
            .get(index)
            .copied()
            .ok_or("invalid search handle")?;
        if self.search_handle(player, object)? != *handle {
            return Err("noncanonical search handle".into());
        }
        Ok(object)
    }

    pub(super) fn staple_cause(frame: &Resolving, source_block: u32) -> BatchCause {
        BatchCause::Instruction {
            stack_id: frame.spell.id,
            player: frame.spell.controller,
            source: frame.spell.source,
            cursor: frame.cursor,
            source_block,
        }
    }
    pub(super) fn execute_staples_instruction(
        &mut self,
        frame: Resolving,
        instruction: Instruction,
    ) -> Result<(), String> {
        match instruction {
            Instruction::DestroyTargetCreature {
                target_slot,
                source_block,
            } => {
                let target = frame.spell.targets[target_slot as usize];
                if frame.legal_targets.get(target_slot as usize) == Some(&true)
                    && self.is_battlefield_creature(target)
                {
                    return self.complete_graveyard_group(
                        Self::staple_cause(&frame, source_block),
                        EventGroup::Occurrence {
                            proposed: ProposedEvent::DestroyCreature { object: target },
                        },
                        GroupFinish::Instruction {
                            frame,
                            result: GroupResult::Effect,
                        },
                    );
                }
                self.finish_instruction(frame, InstructionOutcome::Effect { actual: None })
            }
            Instruction::DestroyAllCreatures { source_block } => {
                self.move_board_set(frame, ObjectSet::Creatures, SetMove::Destroy, source_block)
            }
            Instruction::ReturnOwnGraveyardCreatureToBattlefield {
                target_slot,
                source_block,
            } => {
                let target = frame.spell.targets[target_slot as usize];
                if frame.legal_targets.get(target_slot as usize) != Some(&true)
                    || !self.object_target_legal(&frame.spell, target)
                {
                    return self
                        .finish_instruction(frame, InstructionOutcome::IllegalTarget { target });
                }
                if self.begin_copy_entry(&frame, target)? {
                    return Ok(());
                }
                let movement = self.move_object_for(
                    target,
                    Zone::Battlefield,
                    Self::staple_cause(&frame, source_block),
                )?;
                self.finish_instruction(frame, InstructionOutcome::Moved { movement })
            }
            Instruction::ReturnOwnGraveyardCreatureToHand {
                target_slot,
                optional,
                source_block,
            }
            | Instruction::ReturnOwnGraveyardCardToHand {
                target_slot,
                optional,
                source_block,
            } => {
                let target = frame.spell.targets[target_slot as usize];
                if frame.legal_targets.get(target_slot as usize) != Some(&true)
                    || !self.object_target_legal(&frame.spell, target)
                {
                    return self
                        .finish_instruction(frame, InstructionOutcome::IllegalTarget { target });
                }
                if optional && self.is_live(frame.spell.controller) {
                    self.state.rules.frame = Frame::ResolutionChoice {
                        choice: ResolutionChoice::OptionalRecursion {
                            player: frame.spell.controller,
                            target,
                            source_block,
                        },
                        frame,
                    };
                    Ok(())
                } else {
                    self.return_graveyard_card(frame, target, source_block)
                }
            }
            Instruction::SearchColoredLibrary { source_block, .. } => {
                let player = frame.spell.controller;
                self.state.rules.frame = Frame::ResolutionChoice {
                    frame,
                    choice: ResolutionChoice::ColoredSearch {
                        player,
                        source_block,
                        revealed: None,
                    },
                };
                Ok(())
            }
            op @ (Instruction::SearchBasicLands { .. }
            | Instruction::SearchBasicLandsSplit { .. }
            | Instruction::SearchTypedLands { .. }) => {
                let descriptor =
                    LandSearch::instruction(&op).ok_or("invalid land search instruction")?;
                let (maximum, source_block, split) = (
                    descriptor.maximum,
                    descriptor.source_block,
                    descriptor.split,
                );
                if !self.is_live(frame.spell.controller) {
                    return self
                        .finish_instruction(frame, InstructionOutcome::Effect { actual: None });
                }
                self.state.rules.frame = Frame::ResolutionChoice {
                    choice: ResolutionChoice::Search {
                        player: frame.spell.controller,
                        maximum,
                        split,
                        source_block,
                    },
                    frame,
                };
                Ok(())
            }
            _ => Err("not a staples instruction".into()),
        }
    }
    fn return_graveyard_card(
        &mut self,
        frame: Resolving,
        target: ObjectId,
        block: u32,
    ) -> Result<(), String> {
        if !self.object_target_legal(&frame.spell, target) {
            return self
                .finish_instruction(frame, InstructionOutcome::ObjectNoLongerPresent { target });
        }
        self.return_target_to_owner_hand(frame, target, block)
    }
    pub(super) fn return_target_to_owner_hand(
        &mut self,
        frame: Resolving,
        target: ObjectId,
        block: u32,
    ) -> Result<(), String> {
        if self.current(target).is_err() {
            return self
                .finish_instruction(frame, InstructionOutcome::ObjectNoLongerPresent { target });
        }
        let owner = self.state.rules.cards[&target.card].owner;
        // CR903.9b offers the owner the hand-move replacement from ANY zone.
        // Declining leaves an ordinary hand card (still the physical Commander).
        if self.state.rules.commanders.history(target.card).is_some() {
            self.state.rules.frame = Frame::CommanderReturn {
                object: target,
                destination: Zone::Hand { owner },
                continuation: Box::new(CommanderContinuation::SpellMove {
                    frame,
                    source_block: block,
                }),
            };
            return Ok(());
        }
        let movement = self.move_object_for(
            target,
            Zone::Hand { owner },
            Self::staple_cause(&frame, block),
        )?;
        self.finish_instruction(frame, InstructionOutcome::Moved { movement })
    }
    pub(super) fn validate_staples_response(
        &self,
        player: PlayerId,
        response: &Response,
    ) -> Option<Result<(), String>> {
        let Frame::ResolutionChoice { choice, .. } = &self.state.rules.frame else {
            return None;
        };
        match (choice, response) {
            (
                ResolutionChoice::SearchSplit {
                    player: p, objects, ..
                },
                Response::SearchLandBattlefield { object },
            ) => Some(
                if *p == player
                    && self.is_live(player)
                    && objects.len() == 2
                    && objects.contains(object)
                    && objects
                        .iter()
                        .all(|o| self.search_lands(player).contains(o))
                {
                    Ok(())
                } else {
                    Err("invalid revealed land partition".into())
                },
            ),
            (
                ResolutionChoice::Search {
                    player: p, maximum, ..
                },
                Response::SearchLands { objects },
            ) => Some(
                if *p == player
                    && self.is_live(player)
                    && self
                        .active_land_search()
                        .is_some_and(|s| s.maximum == *maximum)
                    && objects.len() <= *maximum as usize
                    && objects.iter().collect::<BTreeSet<_>>().len() == objects.len()
                    && objects
                        .iter()
                        .all(|o| self.search_lands(player).contains(o))
                {
                    Ok(())
                } else {
                    Err("invalid private search choice".into())
                },
            ),
            (
                ResolutionChoice::OptionalRecursion { player: p, .. },
                Response::OptionalRecursion { .. },
            ) => Some(if *p == player && self.is_live(player) {
                Ok(())
            } else {
                Err("invalid optional recursion choice".into())
            }),
            _ => None,
        }
    }
    pub(super) fn staples_choice_step(
        &mut self,
        frame: Resolving,
        choice: ResolutionChoice,
    ) -> Result<(), String> {
        let player = match choice {
            ResolutionChoice::SearchSplit { player, .. }
            | ResolutionChoice::Search { player, .. }
            | ResolutionChoice::Shuffle { player, .. }
            | ResolutionChoice::OptionalRecursion { player, .. }
            | ResolutionChoice::OptionalDraw { player, .. } => player,
            _ => return Err("not staples choice".into()),
        };
        if !self.is_live(player) {
            self.state.rules.pending = None;
            return self
                .finish_instruction(frame, InstructionOutcome::PlayerActionSkipped { player });
        }
        match choice {
            ResolutionChoice::Shuffle {
                index,
                source_block,
                ..
            } => {
                if index == 0 {
                    self.commit(
                        ProposedEvent::FinishLibraryShuffle { player },
                        Self::staple_cause(&frame, source_block),
                    )?;
                    if matches!(
                        self.program_for(&frame.spell)?
                            .instructions
                            .get(frame.cursor as usize),
                        Some(Instruction::RevealAndCast { .. })
                    ) {
                        self.state.rules.frame = Frame::RevealAndCast {
                            progress: Box::new(RevealAndCastProgress::after_shuffle(
                                frame,
                                source_block,
                            )),
                        };
                        return Ok(());
                    }
                    return self
                        .finish_instruction(frame, InstructionOutcome::Effect { actual: None });
                }
                let actual = self.commit(
                    ProposedEvent::ShuffleLibraryWord { player, index },
                    Self::staple_cause(&frame, source_block),
                )?;
                let EventOutcome::LibraryShuffledWord { selected, .. } = actual else {
                    return Err("missing shuffle result".into());
                };
                self.state.rules.frame = Frame::ResolutionChoice {
                    frame,
                    choice: ResolutionChoice::Shuffle {
                        player,
                        index: if selected.is_some() { index - 1 } else { index },
                        source_block,
                    },
                };
                Ok(())
            }
            ResolutionChoice::Search {
                source_block,
                split,
                ..
            } => {
                let input = self
                    .state
                    .rules
                    .pending
                    .take()
                    .ok_or("missing search input")?;
                self.validate_staples_response(input.player, &input.response)
                    .ok_or("wrong search response")??;
                let Response::SearchLands { mut objects } = input.response else {
                    return Err("search response invariant".into());
                };
                if split {
                    objects.sort();
                    self.commit(
                        ProposedEvent::RevealSearchLands {
                            player,
                            objects: objects.clone(),
                        },
                        Self::staple_cause(&frame, source_block),
                    )?;
                    self.state.rules.frame = Frame::ResolutionChoice {
                        frame,
                        choice: ResolutionChoice::SearchSplit {
                            player,
                            objects,
                            source_block,
                        },
                    };
                    return Ok(());
                }
                // All found cards enter together; response order is not semantic.
                objects.sort();
                if !objects.is_empty()
                    && self.begin_land_entry(
                        EntryOrigin::Search {
                            frame: frame.clone(),
                        },
                        objects.clone(),
                    )?
                {
                    return Ok(());
                }
                if !objects.is_empty() {
                    self.commit_batch(
                        Self::staple_cause(&frame, source_block),
                        EventGroup::Simultaneous {
                            occurrences: objects
                                .into_iter()
                                .map(|object| ProposedEvent::SearchLandEntry { object, player })
                                .collect(),
                        },
                    )?;
                }
                self.begin_player_library_shuffle(frame, player, source_block)
            }
            ResolutionChoice::SearchSplit {
                objects,
                source_block,
                ..
            } => {
                let battlefield = if objects.len() == 2 {
                    let input = self
                        .state
                        .rules
                        .pending
                        .take()
                        .ok_or("missing partition input")?;
                    self.validate_staples_response(input.player, &input.response)
                        .ok_or("wrong partition response")??;
                    let Response::SearchLandBattlefield { object } = input.response else {
                        return Err("partition response invariant".into());
                    };
                    Some(object)
                } else {
                    objects.first().copied()
                };
                if let Some(battlefield) = battlefield {
                    // CR 608.2c, 121.5: partition by identity; the hand move is
                    // not a draw. Both moves share the common-prior event group.
                    self.commit_batch(
                        Self::staple_cause(&frame, source_block),
                        EventGroup::Simultaneous {
                            occurrences: objects
                                .into_iter()
                                .map(|object| {
                                    if object == battlefield {
                                        ProposedEvent::SearchLandEntry { object, player }
                                    } else {
                                        ProposedEvent::Move {
                                            object,
                                            destination: Zone::Hand { owner: player },
                                        }
                                    }
                                })
                                .collect(),
                        },
                    )?;
                }
                self.begin_player_library_shuffle(frame, player, source_block)
            }
            ResolutionChoice::OptionalDraw { source_block, .. } => {
                let input = self
                    .state
                    .rules
                    .pending
                    .take()
                    .ok_or("missing optional draw input")?;
                self.validate_actions_response(input.player, &input.response)
                    .ok_or("wrong optional draw response")??;
                if matches!(input.response, Response::OptionalEffect { perform: true }) {
                    self.draw_one(frame, player, source_block)
                } else {
                    self.finish_instruction(frame, InstructionOutcome::Effect { actual: None })
                }
            }
            ResolutionChoice::OptionalRecursion {
                target,
                source_block,
                ..
            } => {
                let input = self
                    .state
                    .rules
                    .pending
                    .take()
                    .ok_or("missing optional recursion input")?;
                self.validate_staples_response(input.player, &input.response)
                    .ok_or("wrong optional response")??;
                if matches!(
                    input.response,
                    Response::OptionalRecursion { perform: true }
                ) {
                    self.return_graveyard_card(frame, target, source_block)
                } else {
                    self.finish_instruction(frame, InstructionOutcome::Effect { actual: None })
                }
            }
            _ => Err("not staples choice".into()),
        }
    }
    pub(super) fn apply_search_shuffle_word(
        &mut self,
        player: PlayerId,
        index: u32,
        replacement_offset: Option<usize>,
    ) -> Result<EventOutcome, String> {
        let offset = if let Some(offset) = replacement_offset {
            offset
        } else if matches!(self.state.rules.frame, Frame::ResolutionChoice { choice: ResolutionChoice::Shuffle { player: p,index: i,.. },.. } if p == player && i==index)
        {
            0
        } else {
            return Err("shuffle continuation invariant".into());
        };
        let library = self
            .state
            .rules
            .libraries
            .get_mut(&player)
            .ok_or("missing search library")?;
        if index == 0 || index as usize >= library.len().saturating_sub(offset) {
            return Err("shuffle range".into());
        }
        // A tucked commander can bring the physical library to all 100 owned
        // cards. The shared sampler uses that common bound, not setup's 99.
        // A countered copy or a tucked token may share a library with all
        // physical cards until the next SBA check (CR 704.5d/e).
        let limit = if replacement_offset.is_some() {
            capacities::OWNED_LIMIT + capacities::TOKEN_CAPACITY as usize
        } else {
            capacities::OWNED_LIMIT
        };
        if library.len() > limit {
            return Err("shuffle library exceeds the physical-card sampler range".into());
        }
        let random = &mut self
            .state
            .rules
            .match_state
            .as_mut()
            .ok_or("missing trusted match randomness")?
            .random;
        let before = random.word_position;
        let word = random.word()?;
        let selected = setup::sample_bounded_word(word, index + 1);
        if let Some(j) = selected {
            library.swap(offset + index as usize, offset + j as usize);
        }
        Ok(EventOutcome::LibraryShuffledWord {
            player,
            index,
            word,
            selected,
            before,
            after: random.word_position,
        })
    }
    pub(super) fn valid_staples_choice(
        &self,
        frame: &Resolving,
        choice: &ResolutionChoice,
        instruction: &Instruction,
    ) -> bool {
        match (choice, instruction) {
            (
                ResolutionChoice::ColoredSearch {
                    player,
                    source_block,
                    revealed,
                },
                Instruction::SearchColoredLibrary {
                    source_block: b,
                    colors,
                },
            ) => {
                *player == frame.spell.controller
                    && b == source_block
                    && self.valid_colored_reveal(*player, colors, revealed.as_deref())
            }
            (
                ResolutionChoice::OptionalDraw {
                    player,
                    source_block,
                },
                Instruction::OptionalDrawForSoleGreatestEntrant {
                    source_block: expected,
                },
            ) => {
                source_block == expected
                    && self.is_live(*player)
                    && frame
                        .entrant_draw
                        .as_ref()
                        .is_some_and(|entrant| entrant.controller == *player)
            }
            (
                ResolutionChoice::OptionalDraw {
                    player,
                    source_block,
                },
                Instruction::OptionalDrawCards {
                    source_block: expected,
                }
                | Instruction::OptionalDrawUnlessCasterPays {
                    source_block: expected,
                    ..
                },
            ) => *player == frame.spell.controller && source_block == expected,
            (
                ResolutionChoice::OptionalEffect {
                    player,
                    target,
                    source_block,
                },
                Instruction::ReturnEntryTargetCreature {
                    target_slot,
                    optional: true,
                    source_block: b,
                }
                | Instruction::DestroyPermanent {
                    target_slot,
                    optional: true,
                    source_block: b,
                    ..
                },
            ) => {
                *player == frame.spell.controller
                    && source_block == b
                    && frame.spell.targets.get(*target_slot as usize) == Some(target)
            }
            (
                ResolutionChoice::Search {
                    player,
                    maximum,
                    split,
                    source_block,
                },
                instruction,
            ) => {
                *player == frame.spell.controller
                    && LandSearch::instruction(instruction).is_some_and(|s| {
                        *split == s.split
                            && *maximum == s.maximum
                            && *source_block == s.source_block
                    })
            }
            (
                ResolutionChoice::SearchSplit {
                    player,
                    objects,
                    source_block,
                },
                Instruction::SearchBasicLandsSplit { source_block: b },
            ) => {
                *player == frame.spell.controller
                    && source_block == b
                    && objects.len() <= 2
                    && objects.windows(2).all(|w| w[0] < w[1])
                    && objects
                        .iter()
                        .all(|o| self.search_lands(*player).contains(o))
            }
            (
                ResolutionChoice::Shuffle {
                    player,
                    index,
                    source_block,
                },
                Instruction::ShufflePlayerGraveyard { source_block: b },
            ) => {
                source_block == b
                    && frame.spell.entity_targets.first()
                        == Some(&TargetEntity::Player { player: *player })
                    && self
                        .graveyard_legality(frame)
                        .is_ok_and(|(legal, _)| legal.first() == Some(&true))
                    && (*index as usize)
                        < self
                            .state
                            .rules
                            .libraries
                            .get(player)
                            .map_or(1, Vec::len)
                            .max(1)
            }
            (
                ResolutionChoice::Shuffle {
                    player,
                    index,
                    source_block,
                },
                Instruction::SearchBasicLands {
                    source_block: b, ..
                }
                | Instruction::SearchBasicLandsSplit { source_block: b }
                | Instruction::SearchTypedLands {
                    source_block: b, ..
                }
                | Instruction::SearchColoredLibrary {
                    source_block: b, ..
                }
                | Instruction::RevealAndCast { source_block: b }
                | Instruction::OptionalShuffleLibrary { source_block: b },
            ) => {
                *player == frame.spell.controller
                    && source_block == b
                    && (*index as usize)
                        < self
                            .state
                            .rules
                            .libraries
                            .get(player)
                            .map_or(1, Vec::len)
                            .max(1)
            }
            (
                ResolutionChoice::OptionalRecursion {
                    player,
                    target,
                    source_block,
                },
                Instruction::ReturnOwnGraveyardCreatureToHand {
                    target_slot,
                    optional: true,
                    source_block: b,
                }
                | Instruction::ReturnOwnGraveyardCardToHand {
                    target_slot,
                    optional: true,
                    source_block: b,
                },
            ) => {
                *player == frame.spell.controller
                    && source_block == b
                    && frame.spell.targets.get(*target_slot as usize) == Some(target)
            }
            _ => false,
        }
    }
}
