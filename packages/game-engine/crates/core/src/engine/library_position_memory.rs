use super::*;

impl RulesTxn {
    pub(crate) fn update_library_position_memory(
        &mut self,
        event: &ProposedEvent,
        actual: &EventOutcome,
    ) {
        let revealed_move = matches!(actual, EventOutcome::Moved { movement }
            if self.replacement_reveals(movement.previous));
        let objects = &self.state.rules.objects;
        for knowledge in &mut self.state.knowledge.visibility {
            // A secret choice from hand into a hidden zone cannot identify
            // which previously revealed card stayed. Forget that hand's rows.
            let concealed_hand = match (event, actual) {
                (ProposedEvent::Move { .. }, EventOutcome::Moved { movement }) => {
                    match (movement.from, movement.destination) {
                        // A printed replacement has already identified the
                        // card publicly. Other known hand cards stay known.
                        (Zone::Hand { owner }, Zone::Library { .. }) if !revealed_move => {
                            Some(owner)
                        }
                        _ => None,
                    }
                }
                _ => None,
            };
            if let Some(owner) = concealed_hand {
                knowledge.revealed_hand.retain(|id| {
                    self.state
                        .rules
                        .cards
                        .get(&id.card)
                        .is_none_or(|c| c.owner != owner)
                });
            }
            knowledge.revealed_hand.retain(|id| {
                objects
                    .get(&id.card)
                    .is_some_and(|o| o.id == *id && matches!(o.zone, Zone::Hand { .. }))
            });
            knowledge.public_tucked_cards.retain(|id| {
                let Some(Object {
                    id: current,
                    zone: Zone::Library { owner },
                    ..
                }) = objects.get(&id.card)
                else {
                    return false;
                };
                if current != id {
                    return false;
                }
                match event {
                    ProposedEvent::FinishLibraryShuffle { player }
                    | ProposedEvent::ShuffleLibraryWord { player, .. } => player != owner,

                    // Scry can hide the size of the portion sent to the
                    // bottom. Opponents cannot update positions from that count.

                    // The chooser sees the reordered prefix. Other viewers
                    // can still track the untouched tail through these actions,
                    // whose order-preserving shift follows from public sizes.
                    ProposedEvent::Setup { .. } => false,
                    _ => true,
                }
            });
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
        for seat in 0..4 {
            self.publicly_searched_hand(PlayerId(seat))?;
            self.known_library_positions(PlayerId(seat))?;
        }
        Ok(())
    }
}

impl RulesTxn {
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
