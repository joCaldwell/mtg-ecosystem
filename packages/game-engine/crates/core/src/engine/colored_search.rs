//! CR 400.7, 701.20a, 701.23b. Quality-constrained searches may find fewer
//! cards; the aggregate is public only after the private allocation is accepted.
use super::*;
use mtg_contracts::{ColoredLibrarySearch, ColoredSearchAssignment};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ColoredCardCandidate {
    pub handle: ObjectHandle,
    pub oracle_id: String,
    pub slots: Vec<Color>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ColoredCardAssignment {
    pub slot: Color,
    pub handle: ObjectHandle,
}

impl Game {
    fn colored_slots<'a>(&'a self, frame: &'a Resolving) -> Result<&'a [Color], String> {
        match self
            .program_for(&frame.spell)?
            .instructions
            .get(frame.cursor as usize)
        {
            Some(Instruction::SearchColoredLibrary { colors, .. }) => Ok(colors),
            _ => Err("colored library search instruction absent".into()),
        }
    }

    fn colored_candidates(
        &self,
        player: PlayerId,
        colors: &[Color],
        decision: u64,
    ) -> Result<Vec<(ObjectId, ColoredCardCandidate)>, String> {
        let mut rows = vec![];
        for o in self
            .state
            .rules
            .objects
            .values()
            .filter(|o| o.zone == Zone::Library { owner: player })
        {
            let d = self.definition(o.id.card)?;
            let slots: Vec<_> = colors
                .iter()
                .copied()
                .filter(|c| d.characteristics.colors.contains(c))
                .collect();
            if !slots.is_empty() {
                rows.push((o.id, d.oracle_id.clone(), slots));
            }
        }
        rows.sort_by(|a, b| (&a.1, a.0).cmp(&(&b.1, b.0)));
        let knowledge = &self.state.knowledge.visibility[player.0 as usize];
        Ok(rows
            .into_iter()
            .enumerate()
            .map(|(n, (id, oracle_id, slots))| {
                (
                    id,
                    ColoredCardCandidate {
                        handle: ObjectHandle(
                            knowledge.scoped(format!("v{}-colored-{decision}-{n}", player.0)),
                        ),
                        oracle_id,
                        slots,
                    },
                )
            })
            .collect())
    }

    pub(super) fn colored_request(
        &self,
        frame: &Resolving,
        player: PlayerId,
    ) -> Result<Request, String> {
        let decision_id = self.state.rules.revision;
        let colors = self.colored_slots(frame)?;
        let candidates = self
            .colored_candidates(player, colors, decision_id)?
            .into_iter()
            .map(|(_, candidate)| candidate)
            .collect();
        Ok(Request::ColoredSearch {
            library: self.search_library_identities(player)?,
            decision_id,
            player,
            colors: colors.to_vec(),
            candidates,
        })
    }

    pub(super) fn colored_found(
        &self,
        frame: &Resolving,
        player: PlayerId,
        assignments: &[ColoredCardAssignment],
        decision: u64,
    ) -> Result<Vec<(Color, ObjectId)>, String> {
        let colors = self.colored_slots(frame)?;
        let search = ColoredLibrarySearch::new(colors)?;
        if assignments.len() > colors.len() {
            return Err("too many colored search assignments".into());
        }
        let candidates = self.colored_candidates(player, colors, decision)?;
        let assigned: Vec<_> = assignments
            .iter()
            .map(|a| {
                let (id, _) = candidates
                    .iter()
                    .find(|(_, c)| c.handle == a.handle)
                    .ok_or("invalid colored library handle")?;
                Ok::<_, String>(ColoredSearchAssignment {
                    slot: a.slot,
                    card: *id,
                })
            })
            .collect::<Result<_, _>>()?;
        let found = search.found(&assigned, |id, slot| {
            candidates
                .iter()
                .any(|(o, c)| o == id && c.slots.contains(&slot))
        })?;
        Ok(found
            .into_iter()
            .map(|id| (assigned.iter().find(|a| a.card == *id).unwrap().slot, *id))
            .collect())
    }

    pub(super) fn valid_colored_reveal(
        &self,
        player: PlayerId,
        colors: &[Color],
        revealed: Option<&[(Color, ObjectId)]>,
    ) -> bool {
        let Ok(search) = ColoredLibrarySearch::new(colors) else {
            return false;
        };
        let Some(revealed) = revealed else {
            return true;
        };
        let rows: Vec<_> = revealed
            .iter()
            .map(|(slot, card)| ColoredSearchAssignment {
                slot: *slot,
                card: *card,
            })
            .collect();
        search
            .found(&rows, |id, slot| {
                self.zone(*id) == Some(Zone::Library { owner: player })
                    && self
                        .definition(id.card)
                        .is_ok_and(|d| d.characteristics.colors.contains(&slot))
            })
            .is_ok_and(|ordered| {
                ordered
                    .into_iter()
                    .copied()
                    .eq(revealed.iter().map(|(_, id)| *id))
            })
    }

    pub(super) fn colored_step(
        &mut self,
        frame: Resolving,
        player: PlayerId,
        source_block: u32,
        revealed: Option<Vec<(Color, ObjectId)>>,
    ) -> Result<(), String> {
        if !self.is_live(player) {
            self.state.rules.pending = None;
            return self
                .finish_instruction(frame, InstructionOutcome::PlayerActionSkipped { player });
        }
        if let Some(revealed) = revealed {
            if !self.valid_colored_reveal(player, self.colored_slots(&frame)?, Some(&revealed)) {
                return Err("stale revealed library cards".into());
            }
            if !revealed.is_empty() {
                // Removing a searched card from an undisclosed position cannot
                // reveal where it was relative to an earlier public tuck.
                let objects = &self.state.rules.objects;
                for knowledge in &mut self.state.knowledge.visibility {
                    knowledge.public_tucked_cards.retain(|id| {
                        !objects
                            .get(&id.card)
                            .is_some_and(|o| o.zone == Zone::Library { owner: player })
                    });
                }
                self.commit_batch(
                    Self::staple_cause(&frame, source_block),
                    EventGroup::Simultaneous {
                        occurrences: revealed
                            .iter()
                            .map(|(_, object)| ProposedEvent::Move {
                                object: *object,
                                destination: Zone::Hand { owner: player },
                            })
                            .collect(),
                    },
                )?;
                for (_, before) in revealed {
                    let successor = self
                        .object_id(before.card)
                        .ok_or("missing found card successor")?;
                    if successor != before
                        && self.zone(successor) == Some(Zone::Hand { owner: player })
                    {
                        for k in &mut self.state.knowledge.visibility {
                            k.revealed_hand.insert(successor);
                        }
                    }
                }
            }
            let index = self
                .state
                .rules
                .libraries
                .get(&player)
                .map_or(0, |l| l.len().saturating_sub(1)) as u32;
            self.state.rules.frame = Frame::ResolutionChoice {
                frame,
                choice: ResolutionChoice::Shuffle {
                    player,
                    index,
                    source_block,
                },
            };
        } else {
            let input = self
                .state
                .rules
                .pending
                .take()
                .ok_or("missing private search selection")?;
            let Response::ColoredSearch { assignments } = input.response else {
                return Err("invalid colored library response".into());
            };
            if input.player != player {
                return Err("wrong colored library chooser".into());
            }
            let found = self.colored_found(&frame, player, &assignments, input.decision_id)?;
            self.state.rules.frame = Frame::ResolutionChoice {
                frame,
                choice: ResolutionChoice::ColoredSearch {
                    player,
                    source_block,
                    revealed: Some(found),
                },
            };
        }
        Ok(())
    }

    /// Publicly known identities in hand. No targetable handle or hand position
    /// is disclosed. A subsequent zone change invalidates that incarnation.
    pub fn publicly_searched_hand(
        &self,
        viewer: PlayerId,
    ) -> Result<Vec<(PlayerId, String)>, String> {
        let k = self
            .state
            .knowledge
            .visibility
            .get(viewer.0 as usize)
            .ok_or("invalid viewer")?;
        let mut cards = vec![];
        for id in &k.revealed_hand {
            let Some(Zone::Hand { owner }) = self.zone(*id) else {
                return Err("stale revealed hand memory".into());
            };
            cards.push((owner, self.definition(id.card)?.oracle_id.clone()));
        }
        cards.sort();
        Ok(cards)
    }
}
