use super::*;

impl Game {
    pub(super) fn remember_public_tuck(&mut self, successor: ObjectId) {
        for knowledge in &mut self.state.knowledge.visibility {
            knowledge.public_tucked_cards.insert(successor);
        }
    }

    pub(super) fn update_library_position_memory(
        &mut self,
        event: &ProposedEvent,
        actual: &EventOutcome,
    ) {
        let objects = &self.state.rules.objects;
        for (seat, knowledge) in self.state.knowledge.visibility.iter_mut().enumerate() {
            // A secret choice from hand into a hidden zone cannot identify
            // which previously revealed card stayed. Forget that hand's rows.
            let concealed_hand = match (event, actual) {
                (
                    ProposedEvent::HandReturn { player, .. },
                    EventOutcome::HandReturned { movements, .. },
                ) if movements.iter().any(|m| {
                    matches!(
                        (m.from, m.destination),
                        (Zone::Hand { .. }, Zone::Library { .. })
                    )
                }) =>
                {
                    Some(*player)
                }
                (ProposedEvent::Move { .. }, EventOutcome::Moved { movement }) => {
                    match (movement.from, movement.destination) {
                        (Zone::Hand { owner }, Zone::Library { .. }) => Some(owner),
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
                    | ProposedEvent::ShuffleLibraryWord { player, .. }
                    | ProposedEvent::SearchLandEntry { player, .. } => player != owner,
                    // Scry can hide the size of the portion sent to the
                    // bottom. Opponents cannot update positions from that count.
                    ProposedEvent::Scry { player, looked, .. } => {
                        player != owner || player.0 as usize == seat || looked.is_empty()
                    }
                    // The chooser sees the reordered prefix. Other viewers
                    // can still track the untouched tail through these actions,
                    // whose order-preserving shift follows from public sizes.
                    ProposedEvent::OrderLibraryTop { player, looked, .. }
                    | ProposedEvent::LibrarySelect { player, looked, .. }
                    | ProposedEvent::Surveil { player, looked, .. } => {
                        player.0 as usize == seat || !looked.contains(id)
                    }
                    ProposedEvent::Setup { .. } => false,
                    _ => true,
                }
            });
        }
    }

    pub(super) fn known_library_positions(
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
