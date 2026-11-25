//! Library verbs: search, shuffle, scry and surveil (CR 701.22-701.25).
//!
//! The acting player looks at hidden library cards through one resolution
//! choice. Only that player's decision names them (`visible_to`), so no
//! other seat learns an identity, a candidate count or an order. Found cards
//! move together; a revealed card bound for its owner's hand stays known to
//! every seat while it remains there (CR 701.23e). A departed actor's library
//! left the game with them (CR 800.4a), so their pending choice is skipped.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::super::*;
use mtg_contracts::kir::{Destination, Node, PlayerExpr, SearchSpec};

/// The bounds of a search: the cards that may be found, in object order, and
/// how many must and may be found (CR 701.23b, d).
pub(in crate::engine) struct SearchBounds {
    pub candidates: Vec<ObjectId>,
    pub minimum: usize,
    pub maximum: usize,
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
                // CR 701.23b, d: nothing to choose when every candidate must
                // be found, including none at all.
                if bounds.minimum == bounds.candidates.len() {
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
                // CR 701.22a, 701.25a: an empty library leaves nothing to look at.
                if looked.is_empty() {
                    return self
                        .finish_instruction(frame, InstructionOutcome::Effect { actual: None });
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

    /// CR 701.23a, e, 110.2: move the found cards together, then record a
    /// printed reveal of the cards now in their owner's hand.
    fn complete_search(
        &mut self,
        frame: Resolving,
        player: PlayerId,
        found: Vec<ObjectId>,
        source_block: u32,
    ) -> Result<(), String> {
        let (verb, _) = self.library_verb(&frame)?;
        let Verb::Search { search, .. } = verb else {
            return Err("search continuation lost its instruction".into());
        };
        if found.is_empty() {
            return self.finish_instruction(
                frame,
                InstructionOutcome::SetMoved {
                    movements: vec![],
                    destroyed_creatures: 0,
                },
            );
        }
        let to = match search.to {
            Destination::Battlefield { tapped, .. } => (!tapped).then_some(Zone::Battlefield),
            Destination::OwnerHand => Some(Zone::Hand { owner: player }),
            Destination::OwnerGraveyard => Some(Zone::Graveyard { owner: player }),
            _ => return Err("search destination has no audited executor".into()),
        };
        // CR 903.9b: a commander put into its owner's hand offers the
        // command zone instead; that replacement has no search continuation.
        if matches!(to, Some(Zone::Hand { .. }))
            && found
                .iter()
                .any(|o| self.state.rules.commanders.history(o.card).is_some())
        {
            return Err("search of a commander into its owner's hand needs CR 903.9b".into());
        }
        let occurrences = found
            .iter()
            .map(|object| match to {
                Some(destination) => ProposedEvent::Move {
                    object: *object,
                    destination,
                },
                None => ProposedEvent::EnterTapped { object: *object },
            })
            .collect();
        let cause = self.instruction_cause(&frame, source_block)?;
        let batch = self.commit_batch(cause.clone(), EventGroup::Simultaneous { occurrences })?;
        let movements = batch
            .occurrences
            .iter()
            .map(|occurrence| match &occurrence.actual {
                EventOutcome::Moved { movement } => Ok(movement.clone()),
                _ => Err("search movement invariant".to_string()),
            })
            .collect::<Result<Vec<_>, _>>()?;
        if search.reveal && matches!(to, Some(Zone::Hand { .. })) {
            self.commit(
                ProposedEvent::RevealFound {
                    player,
                    objects: movements.iter().map(|m| m.successor).collect(),
                },
                cause,
            )?;
        }
        self.finish_instruction(
            frame,
            InstructionOutcome::SetMoved {
                movements,
                destroyed_creatures: 0,
            },
        )
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
        let Frame::ResolutionChoice { frame, choice } = &self.state.rules.frame else {
            return false;
        };
        match choice {
            ResolutionChoice::Scry { player, looked, .. }
            | ResolutionChoice::Surveil { player, looked, .. } => {
                *player == viewer && looked.contains(&object)
            }
            ResolutionChoice::Search { player, .. } if *player == viewer => {
                let Ok((Verb::Search { search, .. }, _)) = self.library_verb(frame) else {
                    return false;
                };
                self.current(object).is_ok_and(|current| {
                    current.id == object
                        && current.zone == (Zone::Library { owner: viewer })
                        && crate::query::matches_pred(
                            &self.object_facts(current),
                            viewer,
                            &search.filter,
                        )
                        .is_yes()
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
        // A departed actor's library left the game; the step skips it.
        if !self.is_live(player) {
            return Ok(true);
        }
        Ok(match (choice, &verb) {
            (ResolutionChoice::Search { .. }, Verb::Search { search, .. }) => {
                let bounds = self.search_bounds(player, search)?;
                bounds.minimum < bounds.candidates.len()
            }
            (
                ResolutionChoice::Scry {
                    looked,
                    bottom,
                    top,
                    ..
                },
                Verb::Scry { amount, .. },
            ) => {
                let amount = self.kernel_amount(frame, amount)?;
                let partition = match (bottom, top) {
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
                };
                partition && !looked.is_empty() && self.library_prefix(player, &amount)? == *looked
            }
            (
                ResolutionChoice::Surveil {
                    looked, graveyard, ..
                },
                Verb::Surveil { amount, .. },
            ) => {
                let amount = self.kernel_amount(frame, amount)?;
                graveyard.as_ref().is_none_or(|graveyard| {
                    Self::subset(looked, graveyard) && Self::rest(looked, graveyard).len() > 1
                }) && !looked.is_empty()
                    && self.library_prefix(player, &amount)? == *looked
            }
            _ => false,
        })
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
