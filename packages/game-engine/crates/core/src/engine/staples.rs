//! Finite staples. Pinned CR 115.1, 400.7, 603.5, 603.10a, 608.2b,
//! 701.8a-b, 701.23a-b/e, 701.24a-b and 903.9a-b:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

pub(super) const LIVE_TOKEN_CAPACITY: u32 = 256;
pub(super) const CREATURE_CARDS: usize = 4 * 28;
pub(super) const CREATURES: usize = CREATURE_CARDS + LIVE_TOKEN_CAPACITY as usize;
pub(super) const DAMAGE_BOUND: usize = 2 * CREATURES;
// Recursion returns creature cards to hand. All creature/recursion casts need
// an empty stack; no flash/reanimation exists. Thus each card enters/dies at
// most once per nonempty-stack episode. At most 36 NEW tokens can enter during
// that episode, in addition to the initial live capacity. This allowance must
// not be lost when dying tokens free capacity. See the contract derivation.
pub(super) const STACK_BOUND: usize = 400 + 4 + 8 * (CREATURES + 36) + CREATURE_CARDS + 4;
pub(super) const EVENT_GROUP_BOUND: usize = 400 + LIVE_TOKEN_CAPACITY as usize + 4;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VisibleSearchLand {
    pub handle: ObjectHandle,
    pub oracle_id: String,
}
impl Game {
    pub(super) fn resolution_draw_player(&self, frame: &Resolving) -> Result<PlayerId, String> {
        match self
            .program_for(&frame.spell)?
            .instructions
            .get(frame.cursor as usize)
        {
            Some(Instruction::DrawCards {
                recipient: Some(LifeRecipient::Target { target_slot }),
                ..
            }) => frame
                .spell
                .player_targets
                .get(*target_slot as usize)
                .copied()
                .ok_or_else(|| "missing draw target".into()),
            Some(Instruction::DrawCards { .. }) => Ok(frame.spell.controller),
            _ => Err("not draw instruction".into()),
        }
    }
    pub(super) fn staples_enabled(&self) -> bool {
        (self.prepared.config.profile == STAPLES_PROFILE
            && self.prepared.catalog.schema_version == 15)
            || self.mana_enabled()
    }
    pub(super) fn graveyard_creatures(&self, player: PlayerId) -> Vec<ObjectId> {
        self.state
            .objects
            .values()
            .filter(|o| {
                o.zone == (Zone::Graveyard { owner: player })
                    && !self.state.tokens.contains_key(&o.id.card)
                    && self
                        .definition(o.id.card)
                        .is_ok_and(|d| d.characteristics.kind == CardKind::Creature)
            })
            .map(|o| o.id)
            .collect()
    }
    pub(super) fn object_target_legal(&self, entry: &StackInstance, object: ObjectId) -> bool {
        match self
            .program_for(entry)
            .ok()
            .and_then(|p| p.targets.first())
            .map(|t| t.domain)
        {
            Some(TargetDomain::OwnGraveyardCreatureCard) => {
                self.graveyard_creatures(entry.controller).contains(&object)
            }
            _ => self.target_legal(object),
        }
    }
    pub(super) fn search_lands(&self, player: PlayerId) -> Vec<ObjectId> {
        // CardId order is physical order, never library order (CR 401.2).
        let mut cards = self
            .state
            .objects
            .values()
            .filter(|o| {
                o.zone == (Zone::Library { owner: player })
                    && self.definition(o.id.card).is_ok_and(|d| {
                        d.characteristics.kind == CardKind::Land && d.characteristics.basic
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
        if !matches!(self.state.frame, Frame::ResolutionChoice { choice: ResolutionChoice::Search { player: p, .. }, .. } if p == player)
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

    fn staple_cause(frame: &Resolving, source_block: u32) -> BatchCause {
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
        if !self.staples_enabled() {
            return Err("unsupported staples profile".into());
        }
        match instruction {
            Instruction::DestroyTargetCreature {
                target_slot,
                source_block,
            } => {
                let target = frame.spell.targets[target_slot as usize];
                if frame.legal_targets.get(target_slot as usize) == Some(&true)
                    && self.target_legal(target)
                {
                    self.commit(
                        ProposedEvent::DestroyCreature { object: target },
                        Self::staple_cause(&frame, source_block),
                    )?;
                }
                self.finish_instruction(frame, InstructionOutcome::Effect { actual: None })
            }
            Instruction::DestroyAllCreatures { source_block } => {
                let occurrences = self
                    .state
                    .objects
                    .values()
                    .filter(|o| self.target_legal(o.id))
                    .map(|o| ProposedEvent::DestroyCreature { object: o.id })
                    .collect::<Vec<_>>();
                if !occurrences.is_empty() {
                    // One group observer snapshot sees EVERY dying source and
                    // subject together (CR 603.10a); tokens cease at next SBA.
                    self.commit_batch(
                        Self::staple_cause(&frame, source_block),
                        EventGroup::Simultaneous { occurrences },
                    )?;
                }
                self.finish_instruction(frame, InstructionOutcome::Effect { actual: None })
            }
            Instruction::ReturnOwnGraveyardCreatureToHand {
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
                    self.state.frame = Frame::ResolutionChoice {
                        choice: ResolutionChoice::OptionalRecursion {
                            player: frame.spell.controller,
                            target,
                            source_block,
                        },
                        frame,
                    };
                    Ok(())
                } else {
                    self.recur_creature(frame, target, source_block)
                }
            }
            Instruction::SearchBasicLands {
                maximum,
                source_block,
            } => {
                if !self.is_live(frame.spell.controller) {
                    return self
                        .finish_instruction(frame, InstructionOutcome::Effect { actual: None });
                }
                self.state.frame = Frame::ResolutionChoice {
                    choice: ResolutionChoice::Search {
                        player: frame.spell.controller,
                        maximum,
                        source_block,
                    },
                    frame,
                };
                Ok(())
            }
            _ => Err("not a staples instruction".into()),
        }
    }
    fn recur_creature(
        &mut self,
        frame: Resolving,
        target: ObjectId,
        block: u32,
    ) -> Result<(), String> {
        if !self.object_target_legal(&frame.spell, target) {
            return self
                .finish_instruction(frame, InstructionOutcome::ObjectNoLongerPresent { target });
        }
        let owner = self.state.cards[&target.card].owner;
        // CR903.9b offers the owner the hand-move replacement from ANY zone.
        // Declining leaves an ordinary hand card (still the physical Commander).
        if self.state.commanders.history(target.card).is_some() {
            self.state.frame = Frame::CommanderReturn {
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
        let Frame::ResolutionChoice { choice, .. } = &self.state.frame else {
            return None;
        };
        match (choice, response) {
            (
                ResolutionChoice::Search {
                    player: p, maximum, ..
                },
                Response::SearchLands { objects },
            ) => Some(
                if self.staples_enabled()
                    && *p == player
                    && self.is_live(player)
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
            ) => Some(
                if self.staples_enabled() && *p == player && self.is_live(player) {
                    Ok(())
                } else {
                    Err("invalid optional recursion choice".into())
                },
            ),
            _ => None,
        }
    }
    pub(super) fn staples_choice_step(
        &mut self,
        frame: Resolving,
        choice: ResolutionChoice,
    ) -> Result<(), String> {
        let player = match choice {
            ResolutionChoice::Search { player, .. }
            | ResolutionChoice::Shuffle { player, .. }
            | ResolutionChoice::OptionalRecursion { player, .. } => player,
            _ => return Err("not staples choice".into()),
        };
        if !self.is_live(player) {
            self.state.pending = None;
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
                self.state.frame = Frame::ResolutionChoice {
                    frame,
                    choice: ResolutionChoice::Shuffle {
                        player,
                        index: if selected.is_some() { index - 1 } else { index },
                        source_block,
                    },
                };
                Ok(())
            }
            ResolutionChoice::Search { source_block, .. } => {
                let input = self.state.pending.take().ok_or("missing search input")?;
                self.validate_staples_response(input.player, &input.response)
                    .ok_or("wrong search response")??;
                let Response::SearchLands { mut objects } = input.response else {
                    return Err("search response invariant".into());
                };
                // All found cards enter together; response order is not semantic.
                objects.sort();
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
                let index = self
                    .state
                    .libraries
                    .get(&player)
                    .map_or(0, |l| l.len().saturating_sub(1)) as u32;
                self.state.frame = Frame::ResolutionChoice {
                    frame,
                    choice: ResolutionChoice::Shuffle {
                        player,
                        index,
                        source_block,
                    },
                };
                Ok(())
            }
            ResolutionChoice::OptionalRecursion {
                target,
                source_block,
                ..
            } => {
                let input = self
                    .state
                    .pending
                    .take()
                    .ok_or("missing optional recursion input")?;
                self.validate_staples_response(input.player, &input.response)
                    .ok_or("wrong optional response")??;
                if matches!(
                    input.response,
                    Response::OptionalRecursion { perform: true }
                ) {
                    self.recur_creature(frame, target, source_block)
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
    ) -> Result<EventOutcome, String> {
        if !self.staples_enabled()
            || !matches!(self.state.frame, Frame::ResolutionChoice { choice: ResolutionChoice::Shuffle { player: p,index: i,.. },.. } if p == player && i==index)
        {
            return Err("shuffle continuation invariant".into());
        }
        let library = self
            .state
            .libraries
            .get_mut(&player)
            .ok_or("missing search library")?;
        if index == 0 || index as usize >= library.len() {
            return Err("shuffle range".into());
        }
        // The setup sampler covers ranges 2..=99, the largest Commander library
        // (100 cards less the commander). Fail loudly rather than retry forever
        // if a future card ever lets a library grow past that bound.
        if library.len() > 99 {
            return Err("shuffle library exceeds the reviewed 99-card sampler range".into());
        }
        let random = &mut self
            .state
            .match_state
            .as_mut()
            .ok_or("missing trusted match randomness")?
            .random;
        let before = random.word_position;
        let word = random.word()?;
        let selected = setup::sample_shuffle_word(word, index + 1);
        if let Some(j) = selected {
            library.swap(index as usize, j as usize);
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
        if !self.staples_enabled() {
            return false;
        }
        match (choice, instruction) {
            (
                ResolutionChoice::Search {
                    player,
                    maximum,
                    source_block,
                },
                Instruction::SearchBasicLands {
                    maximum: m,
                    source_block: b,
                },
            ) => *player == frame.spell.controller && maximum == m && source_block == b,
            (
                ResolutionChoice::Shuffle {
                    player,
                    index,
                    source_block,
                },
                Instruction::SearchBasicLands {
                    source_block: b, ..
                },
            ) => {
                *player == frame.spell.controller
                    && source_block == b
                    && (*index as usize)
                        < self.state.libraries.get(player).map_or(1, Vec::len).max(1)
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
