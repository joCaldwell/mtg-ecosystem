//! CR 401.2, 401.4, 608.2c–d, 701.20a/e, 903.9b: the chooser sees a
//! fixed library prefix. Only an explicit reveal publishes the selection;
//! ordering the residual cards never publishes their identities or order.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use mtg_contracts::kir::{Amount, LookSelection};

impl RulesTxn {
    pub(super) fn look_selection(
        &self,
        frame: &Resolving,
    ) -> Result<(Amount, LookSelection, u32), String> {
        let (
            Verb::LookChooseHand {
                amount, selection, ..
            },
            block,
        ) = self.library_verb(frame)?
        else {
            return Err("missing private library selection".into());
        };
        Ok((amount, selection, block))
    }

    pub(super) fn look_selection_bounds(
        &self,
        player: PlayerId,
        looked: &[ObjectId],
        spec: &LookSelection,
    ) -> SearchBounds {
        let candidates: Vec<_> = looked
            .iter()
            .copied()
            .filter(|id| {
                self.current(*id).is_ok_and(|object| {
                    !self.state.rules.tokens.contains_key(&id.card)
                        && crate::query::matches_pred(
                            &self.object_facts(object),
                            player,
                            &spec.filter,
                        )
                        .is_yes()
                })
            })
            .collect();
        let maximum = (spec.maximum as usize).min(candidates.len());
        SearchBounds {
            candidates,
            minimum: if spec.optional { 0 } else { maximum },
            maximum,
            library: looked.len(),
            public: false,
        }
    }

    pub(super) fn valid_look_answer(
        &self,
        player: PlayerId,
        looked: &[ObjectId],
        selected: &[ObjectId],
        spec: &LookSelection,
    ) -> bool {
        let bounds = self.look_selection_bounds(player, looked, spec);
        (bounds.minimum..=bounds.maximum).contains(&selected.len())
            && Self::subset(&bounds.candidates, selected)
    }

    pub(super) fn advance_look_selection(
        &mut self,
        frame: Resolving,
        player: PlayerId,
        looked: Vec<ObjectId>,
        selected: Vec<ObjectId>,
        source_block: u32,
    ) -> Result<(), String> {
        let (_, spec, _) = self.look_selection(&frame)?;
        if !self.valid_look_answer(player, &looked, &selected, &spec) {
            return Err("invalid private library selection answer".into());
        }
        // Preserve prefix order, independent of the order used to submit the
        // selection. The printed choice is a subset, not a permutation.
        let selected: Vec<_> = looked
            .iter()
            .copied()
            .filter(|id| selected.contains(id))
            .collect();
        if spec.reveal && !selected.is_empty() {
            self.commit(
                ProposedEvent::RevealFound {
                    player,
                    objects: selected.clone(),
                },
                self.instruction_cause(&frame, source_block)?,
            )?;
        }
        let bottom = Self::rest(&looked, &selected);
        if !spec.random_bottom && bottom.len() > 1 {
            self.rules_mut().frame = Frame::ResolutionChoice {
                frame,
                choice: ResolutionChoice::LookOrderBottom {
                    player,
                    looked,
                    selected,
                    source_block,
                },
            };
            Ok(())
        } else {
            self.route_look_selection(
                frame,
                player,
                LookPartition {
                    looked,
                    selected,
                    bottom,
                },
                vec![],
                source_block,
            )
        }
    }

    fn look_commanders(&self, selected: &[ObjectId]) -> Vec<ObjectId> {
        selected
            .iter()
            .copied()
            .filter(|id| self.state.rules.commanders.history(id.card).is_some())
            .collect()
    }

    pub(in crate::engine) fn route_look_selection(
        &mut self,
        frame: Resolving,
        player: PlayerId,
        partition: LookPartition,
        choices: Vec<(ObjectId, bool)>,
        source_block: u32,
    ) -> Result<(), String> {
        if !self.is_live(player) {
            return self
                .finish_instruction(frame, InstructionOutcome::PlayerActionSkipped { player });
        }
        let (spec, commanders) = self
            .validate_look_partition(&frame, player, &partition, &choices, source_block)?
            .ok_or("invalid private library partition")?;
        if let Some(&object) = commanders.get(choices.len()) {
            self.rules_mut().frame = Frame::CommanderReturn {
                object,
                destination: Zone::Hand { owner: player },
                continuation: CommanderContinuation::LookHand {
                    frame,
                    player,
                    partition,
                    choices,
                    source_block,
                },
            };
            return Ok(());
        }
        self.commit_look_selection(frame, player, partition, &choices, &spec, source_block)
    }

    fn validate_look_partition(
        &self,
        frame: &Resolving,
        player: PlayerId,
        partition: &LookPartition,
        choices: &[(ObjectId, bool)],
        source_block: u32,
    ) -> Result<Option<(LookSelection, Vec<ObjectId>)>, String> {
        let (amount, spec, block) = self.look_selection(frame)?;
        let LookPartition {
            looked,
            selected,
            bottom,
        } = partition;
        if block != source_block
            || self.entry(frame.stack_id)?.controller != player
            || self.library_cards_prefix(player, &self.kernel_amount(frame, &amount)?)? != *looked
            || !self.valid_look_answer(player, looked, selected, &spec)
        {
            return Ok(None);
        }
        let rest = Self::rest(looked, selected);
        if Self::rest(looked, &rest) != *selected
            || if spec.random_bottom {
                rest != *bottom
            } else {
                !Self::same_cards(&rest, bottom)
            }
        {
            return Ok(None);
        }
        let commanders = self.look_commanders(selected);
        if choices.len() > commanders.len()
            || !choices
                .iter()
                .zip(&commanders)
                .all(|((answered, _), expected)| answered == expected)
        {
            return Ok(None);
        }
        Ok(Some((spec, commanders)))
    }

    pub(in crate::engine) fn valid_look_commander(
        &self,
        frame: &Resolving,
        player: PlayerId,
        partition: &LookPartition,
        choices: &[(ObjectId, bool)],
        object: ObjectId,
        source_block: u32,
    ) -> Result<bool, String> {
        Ok(self
            .validate_look_partition(frame, player, partition, choices, source_block)?
            .is_some_and(|(_, commanders)| commanders.get(choices.len()) == Some(&object)))
    }

    fn commit_look_selection(
        &mut self,
        frame: Resolving,
        player: PlayerId,
        partition: LookPartition,
        choices: &[(ObjectId, bool)],
        spec: &LookSelection,
        source_block: u32,
    ) -> Result<(), String> {
        let LookPartition {
            looked,
            selected,
            bottom,
        } = partition;
        let before_bottom = Self::rest(&looked, &selected);
        let moves: Vec<_> = selected
            .iter()
            .map(|object| ProposedEvent::Move {
                object: *object,
                destination: if choices.contains(&(*object, true)) {
                    Zone::Command { owner: player }
                } else {
                    Zone::Hand { owner: player }
                },
            })
            .collect();
        let mut children = vec![];
        match moves.as_slice() {
            [] => {}
            [one] => children.push(EventGroup::Occurrence {
                proposed: one.clone(),
            }),
            _ => children.push(EventGroup::Simultaneous { occurrences: moves }),
        }
        if !bottom.is_empty() {
            children.push(EventGroup::Occurrence {
                proposed: ProposedEvent::PutLibraryPrefixOnBottom {
                    player,
                    looked: before_bottom,
                    bottom,
                    random: spec.random_bottom,
                },
            });
        }
        let cause = self.instruction_cause(&frame, source_block)?;
        let (actual, journal) = self.with_actual_journal(|txn| {
            let result_at = vec![(children.len() - 1) as u32];
            let (group, path) = if children.len() == 1 {
                (children.pop().unwrap(), vec![])
            } else {
                (EventGroup::Ordered { children }, result_at)
            };
            let result = txn.commit_batch(cause, group)?;
            Ok(result.result(&path).cloned())
        })?;
        if spec.reveal {
            let hand: Vec<_> = journal
                .actuals
                .iter()
                .filter_map(|actual| {
                    let movement = actual.movement()?;
                    (movement.destination == (Zone::Hand { owner: player }))
                        .then_some(movement.successor)
                })
                .collect();
            if !hand.is_empty() {
                self.remember_revealed_hand(player, &hand)?;
            }
        }
        self.finish_instruction(frame, InstructionOutcome::Effect { actual })
    }

    /// CR 401.4, 701.20e: a completed selection can retain its bottomed
    /// complement or, if empty, a singleton movement or no specialist leaf.
    pub(in crate::engine) fn validate_look_selection_outcome(
        &self,
        frame: &Resolving,
        instruction: &Instruction,
        outcome: &InstructionOutcome,
    ) -> Result<(), String> {
        let result = match outcome {
            InstructionOutcome::Effect { actual } => actual.as_ref(),
            _ => None,
        };
        let Some(Verb::LookChooseHand {
            amount, selection, ..
        }) = instruction.verb()
        else {
            return if matches!(result, Some(EventOutcome::LibraryPrefixBottomed { .. })) {
                Err("snapshot bottom placement on another instruction".into())
            } else {
                Ok(())
            };
        };
        let player = self.entry(frame.stack_id)?.controller;
        let prefix_size = self.kernel_amount(frame, amount)?;
        let valid = match result {
            Some(EventOutcome::LibraryPrefixBottomed {
                player: actor,
                looked,
                bottom,
            }) => {
                let count = Quantity::from(looked.len() as i64);
                *actor == player
                    && self.valid_looked(frame, player, looked, amount)?
                    && (selection.optional || count < prefix_size)
                    && Self::same_cards(looked, bottom)
            }
            Some(EventOutcome::Moved { movement }) => {
                movement.from == (Zone::Library { owner: player })
                    && (movement.destination == (Zone::Hand { owner: player })
                        || (movement.destination == (Zone::Command { owner: player })
                            && self
                                .state
                                .rules
                                .commanders
                                .history(movement.previous.card)
                                .is_some()))
            }
            None => {
                matches!(outcome, InstructionOutcome::Effect { actual: None })
                    || matches!(outcome, InstructionOutcome::PlayerActionSkipped { player: actor }
                    if *actor == player && !self.is_live(player))
            }
            _ => false,
        };
        if valid {
            Ok(())
        } else {
            Err("snapshot completed library selection invariant".into())
        }
    }
}
