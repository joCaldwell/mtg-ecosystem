//! CR 303.4f–i, 614.12a, 616.1c, 707.2/5–7 (2026-09-25).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

#[derive(Clone, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntryCopySelection {
    // Only the most recent model supplies copiable values. A model whose own
    // text offers a fresh replacement may be copied, then declined or replaced.
    model: Option<ObjectId>,
    declined: bool,
    attachment: Option<ObjectId>,
}

impl RulesTxn {
    pub(super) fn has_entry_copy(&self, source: ObjectId) -> Result<bool, String> {
        Ok(self
            .copiable_definition(source)?
            .static_abilities
            .iter()
            .any(|a| {
                a.functioning_zone == FunctioningZone::Battlefield
                    && a.effect == StaticEffect::EnterAsCopyOfPermanentEnteredThisTurn
            }))
    }

    pub(super) fn begin_copy_entry(
        &mut self,
        frame: &Resolving,
        source: ObjectId,
    ) -> Result<bool, String> {
        if !self.has_entry_copy(source)? {
            return Ok(false);
        }
        self.begin_land_entry(
            EntryOrigin::Copy {
                frame: frame.clone(),
                selection: EntryCopySelection::default(),
            },
            vec![source],
        )
    }

    fn is_entry_model(&self, id: ObjectId) -> bool {
        self.current(id).is_ok_and(|o| {
            o.present_on_battlefield()
                && o.battlefield_entry_turn == Some(self.state.rules.turn.number)
        })
    }

    pub(super) fn entry_copy_model(&self, source: ObjectId) -> Option<ObjectId> {
        match &self.state.rules.frame {
            Frame::LandEntry {
                origin: EntryOrigin::Copy { selection, .. },
                objects,
                ..
            } if objects.as_slice() == [source] => selection.model,
            _ => None,
        }
    }

    pub(super) fn copy_entry_cause(
        &self,
        frame: &Resolving,
        source: ObjectId,
    ) -> Result<BatchCause, String> {
        let spell = self.entry(frame.stack_id)?;
        let program = self.program_for(spell)?;
        if frame.condition_check.as_ref().is_some_and(|c| !c.passed()) {
            return Err("failed condition cannot cause entry".into());
        }
        match program.instructions.get(frame.cursor as usize) {
            None if frame.cursor as usize == program.instructions.len()
                && spell.kind.is_spell()
                && self.top()? == spell
                && program.resolution == SpellResolution::Permanent
                && spell.source == source
                && self.current(source)?.zone == Zone::Stack
                && self.current(source)?.controller == spell.controller
                && frame.legal_targets.is_empty() =>
            {
                Ok(BatchCause::SpellCompletion {
                    stack_id: spell.id,
                    player: spell.controller,
                    source,
                })
            }

            _ => Err("invalid copy entry producer/source".into()),
        }
    }

    pub(super) fn validate_copy_entry(
        &self,
        frame: &Resolving,
        objects: &[ObjectId],
        selection: &EntryCopySelection,
    ) -> Result<(), String> {
        let [source] = objects else {
            return Err("invalid entry copy group".into());
        };
        self.validate_resolving(frame)?;
        self.copy_entry_cause(frame, *source)?;
        if !self.has_entry_copy(*source)?
            || selection.model.is_some_and(|m| !self.is_entry_model(m))
            || (selection.declined && !self.has_entry_copy(selection.model.unwrap_or(*source))?)
        {
            return Err("invalid entry copy choice".into());
        }
        if let Some(target) = selection.attachment {
            let donor = selection.model.ok_or("attachment without copy model")?;
            if !self
                .entry_aura_targets(donor, self.entry(frame.stack_id)?.controller)?
                .is_some_and(|targets| targets.contains(&target))
            {
                return Err("illegal copied Aura attachment".into());
            }
        }
        Ok(())
    }

    pub(super) fn entry_aura_targets(
        &self,
        donor: ObjectId,
        controller: PlayerId,
    ) -> Result<Option<Vec<ObjectId>>, String> {
        let aura = self.copiable_definition(donor)?;
        if !aura.characteristics.subtypes.iter().any(|s| s == "Aura") {
            return Ok(None);
        }
        let colors = self.battlefield_color_layer(
            std::borrow::Cow::Borrowed(&aura.characteristics),
            controller,
        )?;
        let mut targets = Vec::new();
        for o in self
            .state
            .rules
            .objects
            .values()
            .filter(|o| o.present_on_battlefield())
        {
            // CR 303.4f: no targeting. Protection still prohibits enchanting.
            if self.attachment_target_type_valid(donor, o.id)?
                && !self.protected_from(o.id, &colors)
            {
                targets.push(o.id);
            }
        }
        Ok(Some(targets))
    }

    pub(super) fn copy_entry_request(&self) -> Result<Option<Request>, String> {
        let Frame::LandEntry {
            origin: EntryOrigin::Copy { frame, selection },
            objects,
            ..
        } = &self.state.rules.frame
        else {
            return Ok(None);
        };
        let [source] = objects.as_slice() else {
            return Err("invalid entry copy group".into());
        };
        let player = self.entry(frame.stack_id)?.controller;
        let decision_id = self.state.rules.revision;
        if !selection.declined && self.has_entry_copy(selection.model.unwrap_or(*source))? {
            let candidates: Vec<_> = self
                .state
                .rules
                .objects
                .values()
                .filter(|o| self.is_entry_model(o.id))
                .map(|o| o.id)
                .collect();
            if !candidates.is_empty() {
                return Ok(Some(Request::EntryCopy {
                    player,
                    decision_id,
                    source: *source,
                    candidates,
                }));
            }
        }
        if selection.attachment.is_none()
            && let Some(donor) = selection.model
            && let Some(candidates) = self.entry_aura_targets(donor, player)?
            && !candidates.is_empty()
        {
            return Ok(Some(Request::EntryAttachment {
                player,
                decision_id,
                source: *source,
                candidates,
            }));
        }
        Ok(None)
    }

    pub(super) fn validate_copy_entry_response(
        &self,
        player: PlayerId,
        response: &Response,
        request: &Request,
    ) -> Result<(), String> {
        if request.player() == player
            && match (request, response) {
                (Request::EntryCopy { candidates, .. }, Response::EntryCopy { object }) => {
                    object.is_none_or(|m| candidates.contains(&m))
                }
                (
                    Request::EntryAttachment { candidates, .. },
                    Response::EntryAttachment { object },
                ) => candidates.contains(object),
                _ => false,
            }
        {
            Ok(())
        } else {
            Err("wrong copy entry chooser, phase or incarnation".into())
        }
    }

    pub(super) fn consume_copy_entry_response(&mut self, request: &Request) -> Result<(), String> {
        let input = self
            .rules_mut()
            .pending
            .take()
            .ok_or("missing copy entry choice")?;
        self.validate_copy_entry_response(input.player, &input.response, request)?;
        let Frame::LandEntry {
            origin: EntryOrigin::Copy { selection, .. },
            ..
        } = &mut self.rules_mut().frame
        else {
            return Err("missing copy entry origin".into());
        };
        match input.response {
            Response::EntryCopy {
                object: Some(donor),
            } => selection.model = Some(donor),
            Response::EntryCopy { object: None } => selection.declined = true,
            Response::EntryAttachment { object } => selection.attachment = Some(object),
            _ => return Err("invalid copy entry response".into()),
        }
        Ok(())
    }

    pub(super) fn copy_entry_movement(
        &self,
        source: ObjectId,
        selection: &EntryCopySelection,
    ) -> Result<Option<ProposedEvent>, String> {
        if let Some(target) = selection.attachment {
            return Ok(Some(ProposedEvent::EnterAttached {
                object: source,
                target,
            }));
        }
        if let Some(model) = selection.model
            && self
                .copiable_definition(model)?
                .characteristics
                .subtypes
                .iter()
                .any(|s| s == "Aura")
        {
            // CR 303.4g: a spell goes to its owner's graveyard. A card in any
            // other zone stays there with its existing incarnation.
            return Ok(
                (self.current(source)?.zone == Zone::Stack).then(|| ProposedEvent::Move {
                    object: source,
                    destination: Zone::Graveyard {
                        owner: self.state.rules.cards[&source.card].owner,
                    },
                }),
            );
        }
        Ok(Some(ProposedEvent::Move {
            object: source,
            destination: Zone::Battlefield,
        }))
    }

    pub(super) fn finish_copy_entry(
        &mut self,
        frame: Resolving,
        batch: &CommittedBatch,
    ) -> Result<(), String> {
        match batch.cause {
            BatchCause::SpellCompletion { .. } => {
                self.remove_resolved_entry(frame.stack_id)?;
                Ok(self.complete_spell_resolution(frame.stack_id, true)?)
            }
            BatchCause::Instruction { .. } => {
                let movement = match batch.occurrences.last().map(|r| &r.actual) {
                    Some(
                        EventOutcome::Moved { movement }
                        | EventOutcome::EnteredAttached { movement, .. },
                    ) => movement.clone(),
                    _ => return Err("missing graveyard return movement".into()),
                };
                self.finish_instruction(frame, InstructionOutcome::Moved { movement })
            }
            _ => Err("invalid copy entry completion".into()),
        }
    }

    pub(super) fn revalidate_copy_entry_departure(
        &mut self,
        frame: &Resolving,
        source: ObjectId,
        selection: &EntryCopySelection,
    ) -> Result<(), String> {
        if self.current(source).is_err() || !self.is_live(self.entry(frame.stack_id)?.controller) {
            self.rules_mut().pending = None;
            if self.entry(frame.stack_id)?.source == source {
                return Ok(self.finish_spell(frame.stack_id, true)?);
            }
            return self.finish_instruction(
                frame.clone(),
                InstructionOutcome::ObjectNoLongerPresent { target: source },
            );
        }
        let missing_model = selection.model.is_some_and(|m| !self.is_entry_model(m));
        let missing_attachment =
            if let (Some(model), Some(target)) = (selection.model, selection.attachment) {
                missing_model
                    || !self
                        .entry_aura_targets(model, self.entry(frame.stack_id)?.controller)?
                        .is_some_and(|targets| targets.contains(&target))
            } else {
                false
            };
        if missing_model || missing_attachment {
            let Frame::LandEntry {
                origin: EntryOrigin::Copy { selection, .. },
                choices,
                ..
            } = &mut self.rules_mut().frame
            else {
                return Err("missing copy entry origin".into());
            };
            if missing_model {
                *selection = EntryCopySelection::default();
            } else {
                selection.attachment = None;
            }
            choices.clear();
            self.rules_mut().pending = None;
        }
        Ok(())
    }
}
