use super::*;

impl RulesTxn {
    pub(crate) fn update_library_position_memory(
        &mut self,
        event: &ProposedEvent,
        actual: &EventOutcome,
    ) {
        // A found card can leave from an unknown position. Publishing an
        // observed shift would disclose where the hidden selection was.
        let disturbed_library = match actual {
            EventOutcome::Moved { movement } | EventOutcome::EnteredAttached { movement, .. } => {
                match movement.from {
                    Zone::Library { owner } => Some(owner),
                    _ => None,
                }
            }
            _ => None,
        };
        let objects = &self.state.rules.objects;
        for knowledge in &mut self.state.knowledge.visibility {
            knowledge.public_tucked_cards.retain(|id| {
                let Some(Object {
                    id: current,
                    zone: Zone::Library { owner },
                    ..
                }) = objects.get(&id.card)
                else {
                    return false;
                };
                if current != id || disturbed_library == Some(*owner) {
                    return false;
                }
                match event {
                    ProposedEvent::FinishLibraryShuffle { player }
                    | ProposedEvent::ShuffleLibrary { player }
                    | ProposedEvent::ShuffleLibraryToTop { player, .. }
                    | ProposedEvent::ShuffleLibraryWord { player, .. } => player != owner,

                    // The unchanged tail keeps its known positions. Every
                    // permutation of the private prefix is possible to observers.
                    ProposedEvent::ReorderLibraryTop { looked, .. }
                    | ProposedEvent::PutLibraryPrefixOnBottom { looked, .. } => {
                        looked.len() <= 1 || !looked.contains(id)
                    }

                    // Scry can hide the size of the portion sent to the
                    // bottom. Opponents cannot update positions from that count.
                    // Surveil reorders the kept cards secretly; positions of
                    // that library are forgotten conservatively in both.
                    ProposedEvent::Scry { player, .. } | ProposedEvent::Surveil { player, .. } => {
                        player != owner
                    }

                    // The chooser sees the reordered prefix. Other viewers
                    // can still track the untouched tail through these actions,
                    // whose order-preserving shift follows from public sizes.
                    ProposedEvent::Setup { .. } => false,
                    _ => true,
                }
            });
        }
    }

    /// Only an explicit reveal establishes public knowledge of a found
    /// card's position. The CR 701.24b shuffle excludes it.
    pub(super) fn remember_revealed_library_top(&mut self, object: ObjectId) {
        for knowledge in &mut self.state.knowledge.visibility {
            knowledge.public_tucked_cards.insert(object);
        }
    }

    pub(crate) fn known_library_positions(
        &self,
        viewer: PlayerId,
    ) -> Result<Vec<KnownLibraryPosition>, String> {
        let knowledge = &self.state.knowledge.visibility[viewer.0 as usize].public_tucked_cards;
        let mut rows = Vec::with_capacity(knowledge.len());
        for id in knowledge {
            let Zone::Library { owner } = self.current(*id)?.zone else {
                return Err("position memory must bind a library incarnation".into());
            };
            // Exact identity is private. Visible positions derive only for
            // retained memories, after invalidating secret order changes.
            let index = self
                .state
                .rules
                .libraries
                .get(&owner)
                .and_then(|cards| cards.iter().position(|card| *card == id.card))
                .ok_or("remembered card absent from its library")?;
            rows.push(KnownLibraryPosition {
                library_owner: owner,
                position: u32::try_from(index + 1)
                    .map_err(|_| "library position resource limit")?,
                oracle_id: self
                    .state
                    .rules
                    .cards
                    .get(&id.card)
                    .ok_or("missing remembered card identity")?
                    .oracle_id
                    .clone(),
            });
        }
        rows.sort_by_key(|r| (r.library_owner, r.position));
        Ok(rows)
    }

    pub(super) fn validate_library_position_memory(&self) -> Result<(), String> {
        let views = &self.state.knowledge.visibility;
        if views
            .iter()
            .any(|view| view.public_tucked_cards != views[0].public_tucked_cards)
        {
            return Err("public library positions differ between seats".into());
        }
        for seat in 0..4 {
            self.publicly_searched_hand(PlayerId(seat))?;
            self.known_library_positions(PlayerId(seat))?;
        }
        Ok(())
    }
}

impl RulesTxn {
    /// Hand cards of every owner with publicly known hand identities,
    /// captured before an event so its departures can be classified.
    pub(super) fn known_hand_cards(&self) -> Vec<ObjectId> {
        let owners: BTreeSet<PlayerId> = self
            .state
            .knowledge
            .visibility
            .iter()
            .flat_map(|k| k.revealed_hand.keys().copied())
            .collect();
        if owners.is_empty() {
            return vec![];
        }
        self.state
            .rules
            .objects
            .values()
            .filter(|o| matches!(o.zone, Zone::Hand { owner } if owners.contains(&owner)))
            .map(|o| o.id)
            .collect()
    }

    /// CR 400.2, 400.7, 701.23e: revealed hand knowledge is a minimum count
    /// per owner and Oracle identity, never a private incarnation. A card
    /// that leaves the hand into a public zone names its identity, so one
    /// known copy of it is consumed whichever copy it was. A departure into
    /// a hidden zone could have been any card, so one copy of every known
    /// identity is consumed. A departed owner's hand left the game.
    pub(super) fn forget_departed_hand_cards(
        &mut self,
        before: Vec<ObjectId>,
    ) -> Result<(), String> {
        let mut departures = vec![];
        for id in before {
            let current = self.state.rules.objects.get(&id.card);
            if current.is_some_and(|o| o.id == id && matches!(o.zone, Zone::Hand { .. })) {
                continue;
            }
            let card = self
                .state
                .rules
                .cards
                .get(&id.card)
                .ok_or("missing hand card")?;
            let public = current.is_some_and(|o| {
                self.replacement_reveals(id)
                    || (!matches!(o.zone, Zone::Hand { .. } | Zone::Library { .. })
                        && o.hideaway.is_none()
                        && !o.face_down)
            });
            departures.push((card.owner, public.then(|| card.oracle_id.clone())));
        }
        let live: Vec<_> = (0..4).map(|p| self.is_live(PlayerId(p))).collect();
        for knowledge in &mut self.state.knowledge.visibility {
            for (owner, identity) in &departures {
                let Some(rows) = knowledge.revealed_hand.get_mut(owner) else {
                    continue;
                };
                match identity {
                    Some(oracle) => {
                        if let Some(n) = rows.get_mut(oracle) {
                            *n -= 1;
                        }
                    }
                    None => rows.values_mut().for_each(|n| *n -= 1),
                }
                rows.retain(|_, n| *n > 0);
            }
            knowledge
                .revealed_hand
                .retain(|owner, rows| !rows.is_empty() && live[owner.0 as usize]);
        }
        Ok(())
    }

    /// The publicly known copies of `card`'s identity in its owner's hand.
    /// Every seat holds the same counts.
    pub(super) fn known_hand_count(&self, card: CardId) -> u32 {
        let Some(physical) = self.state.rules.cards.get(&card) else {
            return 0;
        };
        self.state.knowledge.visibility[0]
            .revealed_hand
            .get(&physical.owner)
            .and_then(|rows| rows.get(&physical.oracle_id))
            .copied()
            .unwrap_or(0)
    }

    /// CR 601.2i: a reversed announcement puts the card back where it was
    /// announced from, restoring the known copy its departure consumed.
    pub(super) fn restore_known_hand_copy(&mut self, card: CardId) {
        let physical = &self.state.rules.cards[&card];
        let (owner, oracle) = (physical.owner, physical.oracle_id.clone());
        for knowledge in &mut self.state.knowledge.visibility {
            *knowledge
                .revealed_hand
                .entry(owner)
                .or_default()
                .entry(oracle.clone())
                .or_default() += 1;
        }
    }

    /// CR 701.23e: every player saw `cards` enter `owner`'s hand revealed.
    pub(super) fn remember_revealed_hand(
        &mut self,
        owner: PlayerId,
        cards: &[ObjectId],
    ) -> Result<(), String> {
        let mut identities = vec![];
        for id in cards {
            if self.zone(*id) != Some(Zone::Hand { owner }) {
                return Err("revealed hand memory outside its owner's hand".into());
            }
            identities.push(self.definition(id.card)?.oracle_id.clone());
        }
        for knowledge in &mut self.state.knowledge.visibility {
            let rows = knowledge.revealed_hand.entry(owner).or_default();
            for oracle in &identities {
                *rows.entry(oracle.clone()).or_default() += 1;
            }
        }
        Ok(())
    }

    /// CR 701.20a: every player saw `cards`, distinct cards already in
    /// `owner`'s hand, revealed there. A revealed card may be one already
    /// known, so each identity's known count rises only to the number of
    /// its copies revealed now.
    pub(in crate::engine) fn remember_revealed_hand_cards(
        &mut self,
        owner: PlayerId,
        cards: &[ObjectId],
    ) -> Result<(), String> {
        let mut revealed = BTreeMap::<String, u32>::new();
        for id in cards.iter().collect::<BTreeSet<_>>() {
            if self.zone(*id) != Some(Zone::Hand { owner }) {
                return Err("revealed hand card outside its owner's hand".into());
            }
            *revealed
                .entry(self.definition(id.card)?.oracle_id.clone())
                .or_default() += 1;
        }
        for knowledge in &mut self.state.knowledge.visibility {
            let rows = knowledge.revealed_hand.entry(owner).or_default();
            for (oracle, count) in &revealed {
                let known = rows.entry(oracle.clone()).or_default();
                *known = (*known).max(*count);
            }
        }
        Ok(())
    }

    /// Publicly known identities in hand, one row per known copy. No
    /// targetable handle or hand position is disclosed.
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
        // Every reveal and departure is public, so every seat counts alike.
        if self
            .state
            .knowledge
            .visibility
            .iter()
            .any(|other| other.revealed_hand != k.revealed_hand)
        {
            return Err("revealed hand memory differs between seats".into());
        }
        let mut cards = vec![];
        for (owner, rows) in &k.revealed_hand {
            if !self.is_live(*owner) {
                return Err("revealed hand memory of a departed owner".into());
            }
            for (oracle, known) in rows {
                // The minimum count never exceeds the copies actually there.
                let held = self
                    .state
                    .rules
                    .objects
                    .values()
                    .filter(|o| o.zone == (Zone::Hand { owner: *owner }))
                    .filter(|o| {
                        self.state
                            .rules
                            .cards
                            .get(&o.id.card)
                            .is_some_and(|c| c.oracle_id == *oracle)
                    })
                    .count();
                if *known == 0 || *known as usize > held {
                    return Err("stale revealed hand memory".into());
                }
                cards.extend(std::iter::repeat_n(
                    (*owner, oracle.clone()),
                    *known as usize,
                ));
            }
        }
        cards.sort();
        Ok(cards)
    }
}
