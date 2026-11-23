//! CR 400.7, 614.12 and 707.2/5–7 (2026-09-25):
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

// The donor is the permanent actually chosen; definition_card identifies the
// immutable catalog or generated token definition at the root of the copy.
// No counters, choices, attachments, controller, granted abilities or marked
// damage are part of this identity. The entered object's own CardId continues
// to govern ownership, Commander designation and token status.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntryCopy {
    donor: ObjectId,
    definition_card: CardId,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    back_face: bool,
    entry_sequence: u64,
}

impl RulesTxn {
    fn entry_copy(&self, object: ObjectId) -> Option<&EntryCopy> {
        self.state
            .rules
            .entry_copies
            .get(&object.card)?
            .get(&object.incarnation)
    }

    /// Chosen donor incarnation and immutable physical definition root for
    /// the specified battlefield incarnation, including after its departure.
    pub(crate) fn entry_copy_origin(&self, object: ObjectId) -> Option<(ObjectId, CardId)> {
        self.entry_copy(object)
            .map(|row| (row.donor, row.definition_card))
    }

    fn copiable_definition_card(&self, object: ObjectId) -> CardId {
        self.entry_copy_origin(object)
            .map_or(object.card, |(_, root)| root)
    }

    pub(crate) fn back_face_sequence(&self, object: ObjectId) -> Option<u64> {
        self.state
            .rules
            .back_face_objects
            .get(&object.card)?
            .get(&object.incarnation)
            .copied()
    }

    fn copiable_back_side(&self, object: ObjectId) -> bool {
        self.entry_copy(object).map_or_else(
            || self.back_face_sequence(object).is_some(),
            |record| record.back_face,
        )
    }

    fn back_definition(&self, card: CardId) -> Result<Option<&Definition>, String> {
        let root = self.definition(card)?;
        Ok(root.modal_land_back().or_else(|| root.transform_back()))
    }

    pub(super) fn record_back_face(&mut self, successor: ObjectId) -> Result<(), String> {
        if self.back_definition(successor.card)?.is_none()
            || self.back_face_sequence(successor).is_some()
        {
            return Err("invalid modal back entry".into());
        }
        let sequence = self.state.rules.next_event_sequence;
        let charge = stored_history::payload_charge(&(successor, sequence));
        let requested = self.history_charge()?.saturating_add(charge);
        if requested > capacities::HISTORY_LIMIT {
            return Err(self.capacity_error(RuleFault::HistoryCapacityExceeded {
                capacity: capacities::HISTORY_LIMIT,
                requested,
            }));
        }
        self.rules_mut().retained_auxiliary_charge = self
            .state
            .rules
            .retained_auxiliary_charge
            .saturating_add(charge);
        self.rules_mut()
            .back_face_objects
            .entry(successor.card)
            .or_default()
            .insert(successor.incarnation, sequence);
        Ok(())
    }

    pub(super) fn validate_back_face_objects(&self) -> Result<(), String> {
        for (&card, incarnations) in &self.state.rules.back_face_objects {
            let root = self.definition(card)?;
            if incarnations.is_empty() || self.back_definition(card)?.is_none() {
                return Err("invalid back face history owner".into());
            }
            let mut last = None;
            for (&incarnation, &sequence) in incarnations {
                let object = ObjectId { card, incarnation };
                if (incarnation == 0 && root.transform_faces.is_none())
                    || sequence >= self.state.rules.next_event_sequence
                    || last.is_some_and(|previous| sequence <= previous)
                    || !self.could_be_battlefield_incarnation(object)
                    || self.entry_copy(object).is_some()
                {
                    return Err("invalid retained back face history".into());
                }
                last = Some(sequence);
            }
        }
        Ok(())
    }

    /// Base characteristics and intrinsic abilities of this exact incarnation,
    /// including after departure. This lookup alone does not establish presence
    /// or authorize an action. The observer/stack/event owns its ObjectId.
    pub(crate) fn copiable_definition(
        &self,
        object: impl std::borrow::Borrow<ObjectId>,
    ) -> Result<&Definition, String> {
        let object = *object.borrow();
        let (definition_card, back) = self.entry_copy(object).map_or_else(
            || (object.card, self.back_face_sequence(object).is_some()),
            |copy| (copy.definition_card, copy.back_face),
        );
        let root = self.definition(definition_card)?;
        if back {
            return self
                .back_definition(definition_card)?
                .ok_or_else(|| "missing copied back face".into());
        }
        Ok(root)
    }

    /// The supported changing face has activations only on the front. A
    /// previously announced activation keeps that program after a transform.
    pub(super) fn activation_definition(&self, object: ObjectId) -> Result<&Definition, String> {
        let physical = self.definition(object.card)?;
        if physical.transform_faces.is_some() && self.entry_copy(object).is_none() {
            Ok(physical)
        } else {
            self.copiable_definition(object)
        }
    }

    /// The actual transforming object keeps the front face's mana value.
    /// An ordinary card or token copying only the back face has value zero.
    pub(crate) fn physical_transform_mana_value(&self, object: ObjectId) -> Option<Quantity> {
        let definition = self.definition(object.card).ok()?;
        (definition.transform_faces.is_some() && self.entry_copy(object).is_none())
            .then(|| definition.characteristics.printed_mana_value())
    }

    /// Inside the staged entry leaf, before installing its battlefield
    /// successor. The entry transaction owns effect, turn and choice authority.
    /// Every later incarnation automatically reads its own physical definition.
    pub(crate) fn record_entry_copy(
        &mut self,
        entering: ObjectId,
        donor: ObjectId,
    ) -> Result<(), String> {
        let previous = self
            .state
            .rules
            .objects
            .get(&entering.card)
            .ok_or("missing entering object")?;
        if previous.zone == Zone::Battlefield
            || previous.id.incarnation.checked_add(1) != Some(entering.incarnation)
            || donor.card == entering.card
            || !self.current(donor)?.present_on_battlefield()
            || self.entry_copy(entering).is_some()
        {
            return Err("invalid entry copy incarnations".into());
        }
        // Flatten once, including an already-copied permanent or an immutable
        // token definition (which can itself contain a copy exception).
        let definition_card = self.copiable_definition_card(donor);
        self.definition(definition_card)?;
        let row = EntryCopy {
            donor,
            definition_card,
            back_face: self.copiable_back_side(donor),
            // The existing trace-independent sequence orders even ordered
            // sibling movement leaves in the same batch.
            entry_sequence: self.state.rules.next_event_sequence,
        };
        let charge = stored_history::payload_charge(&(entering, row));
        let requested = self.history_charge()?.saturating_add(charge);
        if requested > capacities::HISTORY_LIMIT {
            return Err(self.capacity_error(RuleFault::HistoryCapacityExceeded {
                capacity: capacities::HISTORY_LIMIT,
                requested,
            }));
        }
        self.rules_mut().retained_auxiliary_charge = self
            .state
            .rules
            .retained_auxiliary_charge
            .saturating_add(charge);
        self.rules_mut()
            .entry_copies
            .entry(entering.card)
            .or_default()
            .insert(entering.incarnation, row);
        Ok(())
    }

    fn could_be_battlefield_incarnation(&self, id: ObjectId) -> bool {
        if let Some(token) = self.state.rules.tokens.get(&id.card) {
            // A created token starts on the battlefield. A permanent spell copy
            // starts on the stack and becomes a token on its first move.
            if id.incarnation != u32::from(token.is_copy()) {
                return false;
            }
        } else if id.incarnation == 0
            && !self
                .prepared
                .config
                .cards
                .iter()
                .any(|setup| setup.id == id.card && setup.zone == Zone::Battlefield)
        {
            return false;
        }
        self.state.rules.cards.contains_key(&id.card)
            && self
                .state
                .rules
                .objects
                .get(&id.card)
                .is_none_or(|current| {
                    current.id.incarnation > id.incarnation
                        || current.id == id && current.zone == Zone::Battlefield
                })
    }

    // Replay authenticates the chosen donor, its eligibility and the exact
    // entry boundary. Check the compact lineage independently without recursive
    // traversal of snapshot data: every link must precede the row it explains.
    pub(crate) fn validate_entry_copy_identities(&self) -> Result<(), String> {
        for (card, rows) in &self.state.rules.entry_copies {
            if rows.is_empty() {
                return Err("invalid entry copy owner".into());
            }
            let mut last_sequence = None;
            for (&incarnation, row) in rows {
                if incarnation == 0
                    || row.donor.card == *card
                    || row.entry_sequence >= self.state.rules.next_event_sequence
                    || last_sequence.is_some_and(|last| last >= row.entry_sequence)
                    || !self.could_be_battlefield_incarnation(ObjectId {
                        card: *card,
                        incarnation,
                    })
                    || !self.could_be_battlefield_incarnation(row.donor)
                    || self
                        .entry_copy(row.donor)
                        .is_some_and(|prior| prior.entry_sequence >= row.entry_sequence)
                    || self.copiable_definition_card(row.donor) != row.definition_card
                    || self.entry_copy(row.donor).map_or_else(
                        || {
                            self.back_face_sequence(row.donor)
                                .is_some_and(|sequence| sequence < row.entry_sequence)
                        },
                        |prior| prior.back_face,
                    ) != row.back_face
                {
                    return Err("invalid entry copy definition lineage".into());
                }
                self.definition(row.definition_card)?;
                last_sequence = Some(row.entry_sequence);
            }
        }
        Ok(())
    }
}
