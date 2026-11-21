use super::*;

// CR 109.5/603.3a/603.10a: the actual move and prior source controller
// determine whether "your graveyard" matches. CR 400.7e/603.6c track exactly
// the successor of that public move; do not find a later incarnation.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
pub(super) fn self_graveyard_successor(
    witness: &TriggerEventWitness,
    source: ObjectId,
    controller: PlayerId,
) -> Option<ObjectId> {
    let TriggerEventWitness::Moved { movement } = witness else {
        return None;
    };
    (movement.previous == source
        && movement.from == Zone::Battlefield
        && movement.destination == (Zone::Graveyard { owner: controller }))
    .then_some(movement.successor)
}

impl Game {
    pub(super) fn is_successor_tuck(&self, frame: &Resolving) -> Result<bool, String> {
        Ok(matches!(
            self.program_for(self.entry(frame.stack_id)?)?
                .instructions
                .get(frame.cursor as usize),
            Some(Instruction::MoveTriggeredSuccessor { .. })
        ))
    }

    pub(super) fn successor_tuck_instruction(
        &self,
        frame: &Resolving,
    ) -> Result<(ObjectId, LibraryPosition, u32), String> {
        let stack = self.entry(frame.stack_id)?;
        let Some(Instruction::MoveTriggeredSuccessor {
            expected_zone: TriggerSuccessorZone::TriggerControllerGraveyard,
            destination: TriggerSuccessorZone::TriggerControllerLibrary,
            position,
            source_block,
        }) = self
            .program_for(stack)?
            .instructions
            .get(frame.cursor as usize)
        else {
            return Err("missing successor movement instruction".into());
        };
        let StackKind::Triggered { witness, .. } = &stack.kind else {
            return Err("successor movement requires an actual trigger".into());
        };
        let successor = self_graveyard_successor(witness, stack.source, stack.controller)
            .ok_or("missing controller-graveyard successor")?;
        Ok((successor, *position, *source_block))
    }

    pub(super) fn resolve_successor_tuck(&mut self, frame: Resolving) -> Result<(), String> {
        let (object, _, source_block) = self.successor_tuck_instruction(&frame)?;
        let owner = self.entry(frame.stack_id)?.controller;
        if self.is_live(owner)
            && self
                .current(object)
                .is_ok_and(|o| o.zone == (Zone::Graveyard { owner }))
            && self.state.rules.commanders.history(object.card).is_some()
        {
            self.state.rules.frame = Frame::CommanderReturn {
                object,
                destination: Zone::Library { owner },
                continuation: Box::new(CommanderContinuation::SpellMove {
                    frame,
                    source_block,
                }),
            };
            return Ok(());
        }
        self.complete_successor_tuck(frame, false)
    }

    pub(super) fn complete_successor_tuck(
        &mut self,
        frame: Resolving,
        to_command: bool,
    ) -> Result<(), String> {
        let (object, position, source_block) = self.successor_tuck_instruction(&frame)?;
        let stack = self.entry(frame.stack_id)?;
        let owner = stack.controller;
        // CR 608.2k/609.3: this move is untargeted and only the original
        // successor in the expected zone is eligible.
        let outcome = if self.is_live(owner)
            && self
                .current(object)
                .is_ok_and(|o| o.zone == (Zone::Graveyard { owner }))
        {
            let cause = BatchCause::Instruction {
                stack_id: stack.id,
                player: owner,
                source: stack.source,
                cursor: frame.cursor,
                source_block,
            };
            let movement = if to_command {
                self.move_object_for(object, Zone::Command { owner }, cause)?
            } else {
                match self.commit(
                    ProposedEvent::PutInLibraryAtPosition {
                        object,
                        player: owner,
                        position,
                    },
                    cause,
                )? {
                    EventOutcome::Moved { movement } => movement,
                    _ => return Err("missing ordered-library movement outcome".into()),
                }
            };
            InstructionOutcome::Moved { movement }
        } else {
            InstructionOutcome::ObjectNoLongerPresent { target: object }
        };
        self.finish_instruction(frame, outcome)
    }

    pub(super) fn apply_successor_tuck(
        &mut self,
        object: ObjectId,
        player: PlayerId,
        position: LibraryPosition,
        before: Option<&PreEventFacts>,
    ) -> Result<EventOutcome, String> {
        if !self.is_live(player)
            || self.current(object)?.zone != (Zone::Graveyard { owner: player })
        {
            return Err("ordered-library move requires the recipient's graveyard successor".into());
        }
        let destination = Zone::Library { owner: player };
        let EventOutcome::Moved { movement } = self.apply_occurrence_with_prior(
            &ProposedEvent::Move {
                object,
                destination,
            },
            before,
        )?
        else {
            return Err("ordered-library movement invariant".into());
        };
        let library = self
            .state
            .rules
            .libraries
            .get_mut(&player)
            .ok_or("missing recipient library")?;
        if library.pop() != Some(object.card) {
            return Err("ordered-library movement must append before placing".into());
        }
        // CR 401.7: use the bottom if the specified position has fewer
        // preceding cards than requested. Neither prefix nor suffix changes order.
        let LibraryPosition::NthFromTop(depth) = position;
        let index = ((depth.get() - 1) as usize).min(library.len());
        library.insert(index, object.card);
        self.remember_public_library_position(movement.successor);
        Ok(EventOutcome::Moved { movement })
    }
}
