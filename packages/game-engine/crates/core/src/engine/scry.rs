//! CR 109.5, 400.7, 608.2c/h, 701.22a-d. Looking and same-zone ordering
//! never create successor incarnations or a priority window.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

impl RulesTxn {
    /// CR 111.6–8, 121.1, 704.5d–e: a tucked token or spell copy can stay
    /// in the library zone until state-based actions, with no card to draw.
    pub(super) fn top_library_card(&self, player: PlayerId) -> Option<CardId> {
        self.state
            .rules
            .libraries
            .get(&player)?
            .iter()
            .copied()
            .find(|card| !self.state.rules.tokens.contains_key(card))
    }

    pub(crate) fn library_prefix(
        &self,
        player: PlayerId,
        amount: &Quantity,
    ) -> Result<Vec<ObjectId>, String> {
        if amount.is_negative() {
            return Err("negative scry amount".into());
        }
        let mut looked = vec![];
        for (index, card) in self
            .state
            .rules
            .libraries
            .get(&player)
            .into_iter()
            .flatten()
            .enumerate()
        {
            if Quantity::from(i64::try_from(index).map_err(|_| "scry index limit")?) >= *amount {
                break;
            }
            let object = self.object_id(*card).ok_or("missing scry library card")?;
            if self.current(object)?.zone != (Zone::Library { owner: player }) {
                return Err("scry library zone invariant".into());
            }
            looked.push(object);
        }
        Ok(looked)
    }
}

impl RulesTxn {
    fn library_is_empty(&self, player: PlayerId) -> bool {
        self.state
            .rules
            .libraries
            .get(&player)
            .is_none_or(|cards| cards.is_empty())
    }

    /// The chooser's memory of the cards they put back, top first.
    fn remembered(&self, cards: &[ObjectId], tag: &str) -> Result<Vec<RememberedCard>, String> {
        cards
            .iter()
            .enumerate()
            .map(|(i, o)| {
                Ok(RememberedCard {
                    memory_ref: format!("{tag}-{i}"),
                    oracle_id: self
                        .state
                        .rules
                        .cards
                        .get(&o.card)
                        .ok_or("library memory source")?
                        .oracle_id
                        .clone(),
                })
            })
            .collect()
    }

    /// `top` and `bottom` partition `looked` exactly, without repetition.
    fn library_partition(looked: &[ObjectId], parts: [&[ObjectId]; 2]) -> Result<(), String> {
        let chosen: BTreeSet<_> = parts.iter().flat_map(|p| p.iter()).collect();
        if parts[0].len() + parts[1].len() != looked.len()
            || chosen.len() != looked.len()
            || looked.iter().any(|o| !chosen.contains(o))
        {
            return Err("library partition differs from the looked cards".into());
        }
        Ok(())
    }

    /// CR 401.2, 701.20e: reorder exactly the privately looked-at
    /// prefix. Only the chooser remembers the resulting order.
    pub(super) fn apply_library_top_reorder(
        &mut self,
        player: PlayerId,
        looked: &[ObjectId],
        top: &[ObjectId],
    ) -> Result<EventOutcome, String> {
        let count = Quantity::from(i64::try_from(looked.len()).map_err(|_| "look limit")?);
        if !self.is_live(player)
            || looked.is_empty()
            || self.library_prefix(player, &count)? != looked
        {
            return Err("invalid private library prefix".into());
        }
        Self::library_partition(looked, [top, &[]])?;
        let library = self
            .rules_mut()
            .libraries
            .get_mut(&player)
            .ok_or("missing library")?;
        for (slot, object) in library.iter_mut().zip(top) {
            *slot = object.card;
        }
        self.remember_library_order(player, top)?;
        Ok(EventOutcome::LibraryTopReordered {
            player,
            looked: looked.to_vec(),
            top: top.to_vec(),
        })
    }

    /// CR 701.22a: put `top` back on top in order and `bottom` beneath the
    /// rest of the library in order.
    pub(super) fn apply_scry(
        &mut self,
        player: PlayerId,
        looked: &[ObjectId],
        top: &[ObjectId],
        bottom: &[ObjectId],
    ) -> Result<EventOutcome, String> {
        let amount = Quantity::from(i64::try_from(looked.len()).map_err(|_| "scry limit")?);
        // CR 701.22d: an empty library still scries, with nothing looked at.
        if !self.is_live(player)
            || (looked.is_empty() && !self.library_is_empty(player))
            || self.library_prefix(player, &amount)? != looked
        {
            return Err("invalid scry prefix".into());
        }
        Self::library_partition(looked, [top, bottom])?;
        if looked.is_empty() {
            return Ok(EventOutcome::Scried {
                player,
                top: vec![],
                bottom: vec![],
            });
        }
        let library = self
            .rules_mut()
            .libraries
            .get_mut(&player)
            .ok_or("missing scry library")?;
        let rest = library.split_off(looked.len());
        *library = top
            .iter()
            .map(|o| o.card)
            .chain(rest)
            .chain(bottom.iter().map(|o| o.card))
            .collect();
        let sequence = self.state.knowledge.visibility[player.0 as usize]
            .own_knowledge
            .len();
        let action = OwnKnowledgeAction::Scry {
            top: self.remembered(top, &format!("memory-{sequence}-top"))?,
            bottom: self.remembered(bottom, &format!("memory-{sequence}-bottom"))?,
        };
        self.remember_library_action(player, action)?;
        Ok(EventOutcome::Scried {
            player,
            top: top.to_vec(),
            bottom: bottom.to_vec(),
        })
    }

    /// CR 701.25a: move `graveyard` to its owner's graveyard and put `top`
    /// back on top in order.
    pub(super) fn apply_surveil(
        &mut self,
        player: PlayerId,
        looked: &[ObjectId],
        top: &[ObjectId],
        graveyard: &[ObjectId],
        before: Option<&PreEventFacts>,
    ) -> Result<EventOutcome, String> {
        let amount = Quantity::from(i64::try_from(looked.len()).map_err(|_| "surveil limit")?);
        // CR 701.25d: an empty library still surveils, with nothing looked at.
        if !self.is_live(player)
            || (looked.is_empty() && !self.library_is_empty(player))
            || self.library_prefix(player, &amount)? != looked
        {
            return Err("invalid surveil prefix".into());
        }
        Self::library_partition(looked, [top, graveyard])?;
        if looked.is_empty() {
            return Ok(EventOutcome::Surveilled {
                player,
                top: vec![],
                movements: vec![],
            });
        }
        // Put the kept cards back on top first; the moved cards leave from
        // wherever they are, which changes no other position.
        let library = self
            .rules_mut()
            .libraries
            .get_mut(&player)
            .ok_or("missing surveil library")?;
        let rest = library.split_off(looked.len());
        *library = top
            .iter()
            .chain(graveyard)
            .map(|o| o.card)
            .chain(rest)
            .collect();
        let movements = self.move_library_cards_to_graveyard(player, graveyard, before, None)?;
        let sequence = self.state.knowledge.visibility[player.0 as usize]
            .own_knowledge
            .len();
        let action = OwnKnowledgeAction::Surveil {
            top: self.remembered(top, &format!("memory-{sequence}-top"))?,
        };
        self.remember_library_action(player, action)?;
        Ok(EventOutcome::Surveilled {
            player,
            top: top.to_vec(),
            movements,
        })
    }

    fn remember_library_order(&mut self, player: PlayerId, top: &[ObjectId]) -> Result<(), String> {
        let sequence = self.state.knowledge.visibility[player.0 as usize]
            .own_knowledge
            .len();
        self.remember_library_action(
            player,
            OwnKnowledgeAction::LibraryOrdered {
                top: self.remembered(top, &format!("memory-{sequence}-top"))?,
            },
        )
    }

    pub(super) fn apply_library_prefix_to_bottom(
        &mut self,
        player: PlayerId,
        looked: &[ObjectId],
        bottom: &[ObjectId],
    ) -> Result<EventOutcome, String> {
        let count = Quantity::from(i64::try_from(looked.len()).map_err(|_| "look limit")?);
        if !self.is_live(player)
            || looked.is_empty()
            || self.library_prefix(player, &count)? != looked
        {
            return Err("invalid library prefix for bottom placement".into());
        }
        Self::library_partition(looked, [bottom, &[]])?;
        let cards = self
            .rules_mut()
            .libraries
            .get_mut(&player)
            .ok_or("missing library")?;
        cards.drain(..looked.len());
        cards.extend(bottom.iter().map(|object| object.card));
        let sequence = self.state.knowledge.visibility[player.0 as usize]
            .own_knowledge
            .len();
        self.remember_library_action(
            player,
            OwnKnowledgeAction::LibrarySelected {
                bottom: self.remembered(bottom, &format!("memory-{sequence}-bottom"))?,
            },
        )?;
        Ok(EventOutcome::LibraryPrefixBottomed {
            player,
            looked: looked.to_vec(),
            bottom: bottom.to_vec(),
        })
    }

    /// CR 701.24a-b: randomize the library with the seeded Fisher-Yates
    /// stream, excluding the found card on a search that puts it on top.
    pub(super) fn apply_library_shuffle(
        &mut self,
        player: PlayerId,
        found: Option<ObjectId>,
    ) -> Result<EventOutcome, String> {
        if !self.is_live(player) {
            return Err("departed library shuffle".into());
        }
        if found.is_some_and(|object| self.zone(object) != Some(Zone::Library { owner: player })) {
            return Err("found card is outside the shuffled library".into());
        }
        let rules = self.rules_mut();
        let library = rules.libraries.entry(player).or_default();
        if library.len() > capacities::OWNED_LIMIT + capacities::TOKEN_CAPACITY as usize {
            return Err("shuffle library exceeds the physical card and token bound".into());
        }
        if let Some(found) = found {
            let index = library
                .iter()
                .position(|card| *card == found.card)
                .ok_or("found card missing from shuffled library")?;
            library.remove(index);
        }
        // CR 111.6, 701.24a, 707.10: only cards participate in the
        // permutation. Noncard objects stay in the zone until SBAs.
        let positions: Vec<_> = library
            .iter()
            .enumerate()
            .filter_map(|(i, card)| (!rules.tokens.contains_key(card)).then_some(i))
            .collect();
        let state = rules
            .match_state
            .as_mut()
            .ok_or("missing trusted match randomness")?;
        if state.random.open.is_some() {
            return Err("library shuffle with an open random stream".into());
        }
        let ordinal = &mut state.library_shuffles[player.0 as usize];
        let stream = setup::stream_id(setup::LIBRARY_SHUFFLE, player, *ordinal)?;
        for index in (1..positions.len()).rev() {
            let upper = u32::try_from(index + 1).map_err(|_| "shuffle range")?;
            let selected = loop {
                let word = state.random.word(stream)?;
                if let Some(selected) = setup::sample_bounded_word(word, upper) {
                    break selected;
                }
            };
            library.swap(positions[index], positions[selected as usize]);
        }
        *ordinal = ordinal
            .checked_add(1)
            .ok_or("public shuffle count overflow")?;
        let count = *ordinal;
        state.random.open = None;
        if let Some(found) = found {
            library.insert(0, found.card);
        }
        self.remember_library_action(player, OwnKnowledgeAction::Shuffled)?;
        if let Some(found) = found {
            self.remember_library_order(player, &[found])?;
        }
        Ok(EventOutcome::LibraryShuffled { player, count })
    }

    /// CR 701.23e: every player sees the found cards, still in `player`'s
    /// library, before they move. The search records what stays known.
    pub(super) fn apply_found_reveal(
        &mut self,
        player: PlayerId,
        objects: &[ObjectId],
    ) -> Result<EventOutcome, String> {
        if objects.is_empty()
            || objects.iter().collect::<BTreeSet<_>>().len() != objects.len()
            || objects.iter().any(|o| {
                self.current(*o).map(|o| o.zone).ok() != Some(Zone::Library { owner: player })
            })
        {
            return Err("revealed found cards must be in the searcher's library".into());
        }
        Ok(EventOutcome::FoundRevealed {
            player,
            objects: objects.to_vec(),
        })
    }
}
