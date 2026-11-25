//! Library verbs: search, shuffle, scry and surveil (CR 701.22-701.25).
//!
//! The acting player looks at hidden library cards through one resolution
//! choice. Only that player sees them (`visible_to`): a searcher sees the
//! whole searched library (CR 701.23a) while only matching cards are
//! selectable, so no other seat learns an identity, a candidate count or an
//! order. Candidates follow object order; view rows, like every permitted
//! view object, follow handle order (handles are minted in object order), so
//! neither follows library position. Printed reveals happen once, in the
//! library; a found commander bound for its owner's hand then offers the
//! command zone (CR 903.9b), and the found cards move together. Revealed cards in their owner's hand stay
//! publicly counted by Oracle identity (CR 701.23e). A departed actor's
//! library left the game with them (CR 800.4a), so their pending choice is
//! skipped.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::super::*;
use mtg_contracts::kir::{Destination, Node, PlayerExpr, SearchSpec};

/// The bounds of a search: the cards that may be found, in object order, and
/// how many must and may be found (CR 701.23b, d).
pub(in crate::engine) struct SearchBounds {
    pub candidates: Vec<ObjectId>,
    pub minimum: usize,
    pub maximum: usize,
    /// The number of cards in the searched library.
    pub library: usize,
}

impl SearchBounds {
    /// CR 701.23a: the searcher looks at every card of a non-empty library,
    /// even when none matches. Only a search that must find every card,
    /// including an empty library, has nothing to decide.
    pub fn needs_choice(&self) -> bool {
        self.library > 0
            && (self.minimum < self.candidates.len() || self.candidates.len() < self.library)
    }
}

impl RulesTxn {
    /// The admitted library verb at this frame's cursor.
    fn library_verb(&self, frame: &Resolving) -> Result<(Verb, u32), String> {
        match self
            .program_for(self.entry(frame.stack_id)?)?
            .instructions
            .get(frame.cursor as usize)
        {
            Some(Instruction::Kernel {
                node: Node::Do(verb),
                source_block,
            }) => Ok((verb.clone(), *source_block)),
            _ => Err("missing library instruction".into()),
        }
    }

    /// The live player whose library the verb acts on, or none when that
    /// player has left the game (CR 800.4a).
    fn library_actor(
        &self,
        frame: &Resolving,
        player: &PlayerExpr,
    ) -> Result<Option<PlayerId>, String> {
        Ok(match self.kernel_players(frame, player)? {
            Players::One(player) if self.is_live(player) => Some(player),
            Players::One(_) | Players::Nobody => None,
            Players::EachOpponent => return Err("library verb for each opponent".into()),
        })
    }

    /// CR 701.23a-d: the cards of `player`'s library matching the search's
    /// printed characteristics, and the found-card bounds.
    pub(in crate::engine) fn search_bounds(
        &self,
        player: PlayerId,
        search: &SearchSpec,
    ) -> Result<SearchBounds, String> {
        let mut candidates = self
            .state
            .rules
            .libraries
            .get(&player)
            .into_iter()
            .flatten()
            .map(|card| self.object_id(*card).ok_or("missing library card"))
            .collect::<Result<Vec<_>, _>>()?;
        let library = candidates.len();
        candidates.retain(|id| {
            self.current(*id).is_ok_and(|object| {
                crate::query::matches_pred(&self.object_facts(object), player, &search.filter)
                    .is_yes()
            })
        });
        // The order of a library is hidden even from the searcher.
        candidates.sort();
        let maximum = search
            .maximum
            .as_fixed()
            .and_then(Quantity::to_u32)
            .ok_or("search maximum has no audited evaluation")? as usize;
        let minimum = if search.filter.0.is_empty() {
            maximum.min(candidates.len())
        } else {
            0
        };
        Ok(SearchBounds {
            maximum: maximum.min(candidates.len()),
            candidates,
            minimum,
            library,
        })
    }

    pub(in crate::engine) fn kernel_library(
        &mut self,
        frame: Resolving,
        verb: Verb,
        source_block: u32,
    ) -> Result<(), String> {
        let player = match &verb {
            Verb::Search { player, .. }
            | Verb::Shuffle { player }
            | Verb::Scry { player, .. }
            | Verb::Surveil { player, .. } => player.clone(),
            _ => return Err("not a library verb".into()),
        };
        let Some(actor) = self.library_actor(&frame, &player)? else {
            let outcome = match self.kernel_players(&frame, &player)? {
                Players::One(player) => InstructionOutcome::PlayerActionSkipped { player },
                _ => InstructionOutcome::Effect { actual: None },
            };
            return self.finish_instruction(frame, outcome);
        };
        match verb {
            Verb::Shuffle { .. } => {
                let actual = self.commit(
                    ProposedEvent::ShuffleLibrary { player: actor },
                    self.instruction_cause(&frame, source_block)?,
                )?;
                self.finish_instruction(
                    frame,
                    InstructionOutcome::Effect {
                        actual: Some(actual),
                    },
                )
            }
            Verb::Search { search, .. } => {
                let bounds = self.search_bounds(actor, &search)?;
                // CR 701.23a, d: nothing to choose when every card must be
                // found, including from an empty library.
                if !bounds.needs_choice() {
                    return self.complete_search(frame, actor, bounds.candidates, source_block);
                }
                self.rules_mut().frame = Frame::ResolutionChoice {
                    frame,
                    choice: ResolutionChoice::Search {
                        player: actor,
                        source_block,
                    },
                };
                Ok(())
            }
            Verb::Scry { ref amount, .. } | Verb::Surveil { ref amount, .. } => {
                let amount = self.kernel_amount(&frame, amount)?;
                let looked = self.library_prefix(actor, &amount)?;
                // CR 701.22b, d, 701.25c, d: a positive scry or surveil of
                // an empty library is still that event, with nothing moved.
                if looked.is_empty() {
                    let event = if matches!(verb, Verb::Scry { .. }) {
                        ProposedEvent::Scry {
                            player: actor,
                            looked,
                            top: vec![],
                            bottom: vec![],
                        }
                    } else {
                        ProposedEvent::Surveil {
                            player: actor,
                            looked,
                            top: vec![],
                            graveyard: vec![],
                        }
                    };
                    return self.commit_look(frame, event, source_block);
                }
                let choice = if matches!(verb, Verb::Scry { .. }) {
                    ResolutionChoice::Scry {
                        player: actor,
                        looked,
                        bottom: None,
                        top: None,
                        source_block,
                    }
                } else {
                    ResolutionChoice::Surveil {
                        player: actor,
                        looked,
                        graveyard: None,
                        source_block,
                    }
                };
                self.rules_mut().frame = Frame::ResolutionChoice { frame, choice };
                Ok(())
            }
            _ => Err("not a library verb".into()),
        }
    }

    /// The audited search at this frame's cursor and the zone its found
    /// cards go to; `None` is the battlefield, tapped (CR 110.2, 614.1c).
    fn search_destination(&self, frame: &Resolving) -> Result<(SearchSpec, Option<Zone>), String> {
        let (verb, _) = self.library_verb(frame)?;
        let Verb::Search { player, search } = verb else {
            return Err("search continuation lost its instruction".into());
        };
        let Players::One(player) = self.kernel_players(frame, &player)? else {
            return Err("search without one player".into());
        };
        let to = match search.to {
            Destination::Battlefield { tapped, .. } => (!tapped).then_some(Zone::Battlefield),
            Destination::OwnerHand => Some(Zone::Hand { owner: player }),
            Destination::OwnerGraveyard => Some(Zone::Graveyard { owner: player }),
            _ => return Err("search destination has no audited executor".into()),
        };
        Ok((search, to))
    }

    /// CR 903.9b: found commanders bound for their owner's hand, in object
    /// order, each of whose owner may choose the command zone instead.
    fn found_commanders(
        &self,
        frame: &Resolving,
        found: &[ObjectId],
    ) -> Result<Vec<ObjectId>, String> {
        let (_, to) = self.search_destination(frame)?;
        Ok(if matches!(to, Some(Zone::Hand { .. })) {
            found
                .iter()
                .copied()
                .filter(|o| self.state.rules.commanders.history(o.card).is_some())
                .collect()
        } else {
            vec![]
        })
    }

    /// CR 701.23a, e, 903.9b: reveal printed found cards once, still in the
    /// library, then ask each found commander's owner where it goes before
    /// the found cards move together.
    fn complete_search(
        &mut self,
        frame: Resolving,
        player: PlayerId,
        found: Vec<ObjectId>,
        source_block: u32,
    ) -> Result<(), String> {
        let (search, _) = self.search_destination(&frame)?;
        if search.reveal && !found.is_empty() {
            self.commit(
                ProposedEvent::RevealFound {
                    player,
                    objects: found.clone(),
                },
                self.instruction_cause(&frame, source_block)?,
            )?;
        }
        let remaining = self.found_commanders(&frame, &found)?;
        self.continue_search(frame, player, found, remaining, vec![], source_block)
    }

    /// Offer the next found commander its command-zone choice, or commit
    /// the search once every commander's owner has chosen.
    pub(in crate::engine) fn continue_search(
        &mut self,
        frame: Resolving,
        player: PlayerId,
        found: Vec<ObjectId>,
        remaining: Vec<ObjectId>,
        choices: Vec<(ObjectId, bool)>,
        source_block: u32,
    ) -> Result<(), String> {
        // CR 800.4a: the searcher's library left the game with them.
        if !self.is_live(player) {
            return self
                .finish_instruction(frame, InstructionOutcome::PlayerActionSkipped { player });
        }
        if let Some(object) = remaining.first().copied() {
            self.rules_mut().frame = Frame::CommanderReturn {
                object,
                destination: Zone::Hand { owner: player },
                continuation: CommanderContinuation::Search {
                    frame,
                    player,
                    found,
                    remaining,
                    choices,
                    source_block,
                },
            };
            return Ok(());
        }
        self.commit_search(frame, player, found, &choices, source_block)
    }

    /// CR 701.23a, e, 110.2, 903.9b: move the (already revealed, when
    /// printed) found cards together, each commander to the zone its owner
    /// chose. Revealed cards that reach the hand stay publicly counted there
    /// by Oracle identity.
    fn commit_search(
        &mut self,
        frame: Resolving,
        player: PlayerId,
        found: Vec<ObjectId>,
        choices: &[(ObjectId, bool)],
        source_block: u32,
    ) -> Result<(), String> {
        let (search, to) = self.search_destination(&frame)?;
        if found.is_empty() {
            return self.finish_instruction(
                frame,
                InstructionOutcome::SetMoved {
                    movements: vec![],
                    destroyed_creatures: 0,
                },
            );
        }
        let cause = self.instruction_cause(&frame, source_block)?;
        let occurrences = found
            .iter()
            .map(|object| {
                let command = choices.iter().any(|(o, yes)| o == object && *yes);
                match to {
                    _ if command => ProposedEvent::Move {
                        object: *object,
                        destination: Zone::Command { owner: player },
                    },
                    Some(destination) => ProposedEvent::Move {
                        object: *object,
                        destination,
                    },
                    None => ProposedEvent::EnterTapped { object: *object },
                }
            })
            .collect();
        let batch = self.commit_batch(cause, EventGroup::Simultaneous { occurrences })?;
        let movements = batch
            .occurrences
            .iter()
            .map(|occurrence| match &occurrence.actual {
                EventOutcome::Moved { movement } => Ok(movement.clone()),
                _ => Err("search movement invariant".to_string()),
            })
            .collect::<Result<Vec<_>, _>>()?;
        if search.reveal {
            let revealed: Vec<_> = movements
                .iter()
                .filter(|m| m.destination == (Zone::Hand { owner: player }))
                .map(|m| m.successor)
                .collect();
            if !revealed.is_empty() {
                self.remember_revealed_hand(player, &revealed)?;
            }
        }
        self.finish_instruction(
            frame,
            InstructionOutcome::SetMoved {
                movements,
                destroyed_creatures: 0,
            },
        )
    }

    /// The suspended search continuation is this instruction's own: the
    /// found cards are a legal answer and the commanders are asked in order.
    pub(in crate::engine) fn valid_search_continuation(
        &self,
        object: ObjectId,
        frame: &Resolving,
        player: PlayerId,
        found: &[ObjectId],
        remaining: &[ObjectId],
        choices: &[(ObjectId, bool)],
        source_block: u32,
    ) -> Result<bool, String> {
        let (verb, block) = self.library_verb(frame)?;
        let Verb::Search {
            player: who,
            search,
        } = &verb
        else {
            return Ok(false);
        };
        if block != source_block
            || !matches!(self.kernel_players(frame, who)?, Players::One(p) if p == player)
            || !self.is_live(player)
        {
            return Ok(false);
        }
        let bounds = self.search_bounds(player, search)?;
        let commanders = self.found_commanders(frame, found)?;
        let asked: Vec<_> = choices.iter().map(|(o, _)| *o).collect();
        Ok(Self::subset(&bounds.candidates, found)
            && (bounds.minimum..=bounds.maximum).contains(&found.len())
            && remaining.first() == Some(&object)
            && asked.iter().chain(remaining).copied().collect::<Vec<_>>() == commanders)
    }

    /// The decision a library choice waits for, or `None` when its actor
    /// has left the game.
    pub(in crate::engine) fn library_request(
        &self,
        frame: &Resolving,
        choice: &ResolutionChoice,
    ) -> Result<Option<Decision<Rules>>, String> {
        let objects = |cards: &[ObjectId]| -> Vec<Candidate<Rules>> {
            cards
                .iter()
                .map(|object| Candidate::Object { object: *object })
                .collect()
        };
        Ok(match choice {
            ResolutionChoice::Search { player, .. } if self.is_live(*player) => {
                let (Verb::Search { search, .. }, _) = self.library_verb(frame)? else {
                    return Err("search choice lost its instruction".into());
                };
                // CR 701.23a: the searcher sees the whole library; only the
                // matching cards are offered, possibly none at all.
                let bounds = self.search_bounds(*player, &search)?;
                Some(self.pick(
                    *player,
                    Reason::SearchLibrary,
                    objects(&bounds.candidates),
                    bounds.minimum as u32,
                    bounds.maximum as u32,
                ))
            }
            ResolutionChoice::Scry {
                player,
                looked,
                bottom,
                top,
                ..
            } if self.is_live(*player) => Some(match (bottom, top) {
                (None, _) => self.pick(
                    *player,
                    Reason::ScryBottom,
                    objects(looked),
                    0,
                    looked.len() as u32,
                ),
                (Some(bottom), None) => {
                    let rest = Self::rest(looked, bottom);
                    self.order(
                        *player,
                        Reason::OrderLibraryTop,
                        objects(&rest),
                        rest.len() as u32,
                    )
                }
                (Some(bottom), Some(_)) => self.order(
                    *player,
                    Reason::OrderLibraryBottom,
                    objects(bottom),
                    bottom.len() as u32,
                ),
            }),
            ResolutionChoice::Surveil {
                player,
                looked,
                graveyard,
                ..
            } if self.is_live(*player) => Some(match graveyard {
                None => self.pick(
                    *player,
                    Reason::SurveilGraveyard,
                    objects(looked),
                    0,
                    looked.len() as u32,
                ),
                Some(graveyard) => {
                    let rest = Self::rest(looked, graveyard);
                    self.order(
                        *player,
                        Reason::OrderLibraryTop,
                        objects(&rest),
                        rest.len() as u32,
                    )
                }
            }),
            _ => None,
        })
    }

    /// The looked-at cards outside `chosen`, top first.
    fn rest(looked: &[ObjectId], chosen: &[ObjectId]) -> Vec<ObjectId> {
        looked
            .iter()
            .filter(|o| !chosen.contains(o))
            .copied()
            .collect()
    }

    /// Accept the answer to a library choice, then ask the next question or
    /// perform the action (CR 701.22a, 701.23a, 701.25a).
    pub(in crate::engine) fn library_choice_step(
        &mut self,
        frame: Resolving,
        choice: ResolutionChoice,
    ) -> Result<(), String> {
        let player = match &choice {
            ResolutionChoice::Search { player, .. }
            | ResolutionChoice::Scry { player, .. }
            | ResolutionChoice::Surveil { player, .. } => *player,
            _ => return Err("not a library choice".into()),
        };
        if self.library_request(&frame, &choice)?.is_none() {
            self.rules_mut().pending = None;
            return self
                .finish_instruction(frame, InstructionOutcome::PlayerActionSkipped { player });
        }
        let input = self
            .rules_mut()
            .pending
            .take()
            .ok_or("missing library choice")?;
        if input.player != player {
            return Err("library choice actor invariant".into());
        }
        let chosen = input.objects()?;
        match choice {
            ResolutionChoice::Search { source_block, .. } => {
                self.complete_search(frame, player, chosen, source_block)
            }
            ResolutionChoice::Scry {
                looked,
                bottom: None,
                source_block,
                ..
            } => self.advance_scry(frame, player, looked, chosen, None, source_block),
            ResolutionChoice::Scry {
                looked,
                bottom: Some(bottom),
                top: None,
                source_block,
                ..
            } => self.advance_scry(frame, player, looked, bottom, Some(chosen), source_block),
            ResolutionChoice::Scry {
                looked,
                top: Some(top),
                source_block,
                ..
            } => self.commit_look(
                frame,
                ProposedEvent::Scry {
                    player,
                    looked,
                    top,
                    bottom: chosen,
                },
                source_block,
            ),
            ResolutionChoice::Surveil {
                looked,
                graveyard: None,
                source_block,
                ..
            } => self.advance_surveil(frame, player, looked, chosen, source_block),
            ResolutionChoice::Surveil {
                looked,
                graveyard: Some(graveyard),
                source_block,
                ..
            } => self.commit_look(
                frame,
                ProposedEvent::Surveil {
                    player,
                    looked,
                    top: chosen,
                    graveyard,
                },
                source_block,
            ),
            _ => Err("not a library choice".into()),
        }
    }

    /// CR 701.22a: an order of one card or none needs no decision.
    fn advance_scry(
        &mut self,
        frame: Resolving,
        player: PlayerId,
        looked: Vec<ObjectId>,
        bottom: Vec<ObjectId>,
        top: Option<Vec<ObjectId>>,
        source_block: u32,
    ) -> Result<(), String> {
        let top = match top {
            Some(top) => top,
            None => {
                let rest = Self::rest(&looked, &bottom);
                if rest.len() > 1 {
                    self.rules_mut().frame = Frame::ResolutionChoice {
                        frame,
                        choice: ResolutionChoice::Scry {
                            player,
                            looked,
                            bottom: Some(bottom),
                            top: None,
                            source_block,
                        },
                    };
                    return Ok(());
                }
                rest
            }
        };
        if bottom.len() > 1 {
            self.rules_mut().frame = Frame::ResolutionChoice {
                frame,
                choice: ResolutionChoice::Scry {
                    player,
                    looked,
                    bottom: Some(bottom),
                    top: Some(top),
                    source_block,
                },
            };
            return Ok(());
        }
        self.commit_look(
            frame,
            ProposedEvent::Scry {
                player,
                looked,
                top,
                bottom,
            },
            source_block,
        )
    }

    /// CR 701.25a: an order of one card or none needs no decision.
    fn advance_surveil(
        &mut self,
        frame: Resolving,
        player: PlayerId,
        looked: Vec<ObjectId>,
        graveyard: Vec<ObjectId>,
        source_block: u32,
    ) -> Result<(), String> {
        let top = Self::rest(&looked, &graveyard);
        if top.len() > 1 {
            self.rules_mut().frame = Frame::ResolutionChoice {
                frame,
                choice: ResolutionChoice::Surveil {
                    player,
                    looked,
                    graveyard: Some(graveyard),
                    source_block,
                },
            };
            return Ok(());
        }
        self.commit_look(
            frame,
            ProposedEvent::Surveil {
                player,
                looked,
                top,
                graveyard,
            },
            source_block,
        )
    }

    fn commit_look(
        &mut self,
        frame: Resolving,
        event: ProposedEvent,
        source_block: u32,
    ) -> Result<(), String> {
        let actual = self.commit(event, self.instruction_cause(&frame, source_block)?)?;
        self.finish_instruction(
            frame,
            InstructionOutcome::Effect {
                actual: Some(actual),
            },
        )
    }

    /// A library card the viewer is choosing among right now.
    pub(in crate::engine) fn library_choice_visible(
        &self,
        viewer: PlayerId,
        object: ObjectId,
    ) -> bool {
        let Frame::ResolutionChoice { choice, .. } = &self.state.rules.frame else {
            return false;
        };
        match choice {
            ResolutionChoice::Scry { player, looked, .. }
            | ResolutionChoice::Surveil { player, looked, .. } => {
                *player == viewer && looked.contains(&object)
            }
            // CR 701.23a: every card of the searched library, matching or
            // not; no position is published.
            ResolutionChoice::Search { player, .. } if *player == viewer => {
                self.current(object).is_ok_and(|current| {
                    current.id == object && current.zone == (Zone::Library { owner: viewer })
                })
            }
            _ => false,
        }
    }

    /// The suspended library choice is the admitted instruction's own: its
    /// actor, block and partial answers agree with the node and the library.
    pub(in crate::engine) fn valid_library_choice(
        &self,
        frame: &Resolving,
        choice: &ResolutionChoice,
    ) -> Result<bool, String> {
        let (verb, block) = self.library_verb(frame)?;
        let (player, source_block) = match choice {
            ResolutionChoice::Search {
                player,
                source_block,
            }
            | ResolutionChoice::Scry {
                player,
                source_block,
                ..
            }
            | ResolutionChoice::Surveil {
                player,
                source_block,
                ..
            } => (*player, *source_block),
            _ => return Ok(false),
        };
        let expected = match &verb {
            Verb::Search { player, .. }
            | Verb::Scry { player, .. }
            | Verb::Surveil { player, .. } => match self.kernel_players(frame, player)? {
                Players::One(player) => player,
                _ => return Ok(false),
            },
            _ => return Ok(false),
        };
        if expected != player || source_block != block {
            return Ok(false);
        }
        // The choice variant must be this verb's own, and its partial
        // answers well formed, before the departure fast path accepts it.
        let staged = match (choice, &verb) {
            (ResolutionChoice::Search { .. }, Verb::Search { .. }) => true,
            (
                ResolutionChoice::Scry {
                    looked,
                    bottom,
                    top,
                    ..
                },
                Verb::Scry { amount, .. },
            ) => {
                self.valid_looked(frame, player, looked, amount)?
                    && match (bottom, top) {
                        (None, None) => true,
                        (Some(bottom), None) => {
                            Self::subset(looked, bottom) && Self::rest(looked, bottom).len() > 1
                        }
                        (Some(bottom), Some(top)) => {
                            bottom.len() > 1
                                && Self::subset(looked, bottom)
                                && Self::same_cards(&Self::rest(looked, bottom), top)
                        }
                        (None, Some(_)) => false,
                    }
            }
            (
                ResolutionChoice::Surveil {
                    looked, graveyard, ..
                },
                Verb::Surveil { amount, .. },
            ) => {
                self.valid_looked(frame, player, looked, amount)?
                    && graveyard.as_ref().is_none_or(|graveyard| {
                        Self::subset(looked, graveyard) && Self::rest(looked, graveyard).len() > 1
                    })
            }
            _ => false,
        };
        if !staged {
            return Ok(false);
        }
        // A departed actor's library left the game; the step skips it.
        if !self.is_live(player) {
            return Ok(true);
        }
        Ok(match (choice, &verb) {
            (ResolutionChoice::Search { .. }, Verb::Search { search, .. }) => {
                self.search_bounds(player, search)?.needs_choice()
            }
            (ResolutionChoice::Scry { looked, .. }, Verb::Scry { amount, .. })
            | (ResolutionChoice::Surveil { looked, .. }, Verb::Surveil { amount, .. }) => {
                let amount = self.kernel_amount(frame, amount)?;
                self.library_prefix(player, &amount)? == *looked
            }
            _ => false,
        })
    }

    /// CR 701.22a, 701.25a: the looked-at cards are distinct cards of the
    /// chooser's own library, at least one and at most the printed amount;
    /// this holds even after the chooser left with that library.
    fn valid_looked(
        &self,
        frame: &Resolving,
        player: PlayerId,
        looked: &[ObjectId],
        amount: &mtg_contracts::kir::Amount,
    ) -> Result<bool, String> {
        let amount = self.kernel_amount(frame, amount)?;
        let count = Quantity::from(i64::try_from(looked.len()).map_err(|_| "look limit")?);
        Ok(!looked.is_empty()
            && count <= amount
            // Distinct physical cards: two incarnations are one card.
            && looked.iter().map(|o| o.card).collect::<BTreeSet<_>>().len() == looked.len()
            && looked.iter().all(|o| {
                self.state
                    .rules
                    .cards
                    .get(&o.card)
                    .is_some_and(|card| card.owner == player)
            }))
    }

    /// Distinct members of `looked`.
    fn subset(looked: &[ObjectId], chosen: &[ObjectId]) -> bool {
        chosen.iter().collect::<BTreeSet<_>>().len() == chosen.len()
            && chosen.iter().all(|o| looked.contains(o))
    }

    fn same_cards(left: &[ObjectId], right: &[ObjectId]) -> bool {
        left.len() == right.len()
            && left.iter().collect::<BTreeSet<_>>() == right.iter().collect::<BTreeSet<_>>()
    }
}
