//! Finite staples. Pinned CR 115.1, 400.7, 603.5, 603.10a, 608.2b,
//! 701.8a-b, 701.23a-b/e, 701.24a-b and 903.9a-b:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

impl RulesTxn {
    pub(super) fn resolution_draw_player(&self, frame: &Resolving) -> Result<PlayerId, String> {
        let instruction = self
            .program_for(self.entry(frame.stack_id)?)?
            .instructions
            .get(frame.cursor as usize)
            .ok_or("missing draw instruction")?;
        self.draw_player_for_instruction(frame, instruction)
    }

    pub(super) fn draw_player_for_instruction(
        &self,
        frame: &Resolving,
        instruction: &Instruction,
    ) -> Result<PlayerId, String> {
        match instruction {
            kernel!(Draw { player, .. }) => match player {
                mtg_contracts::kir::PlayerExpr::EachPlayer => frame
                    .each_player_draw
                    .as_ref()
                    .and_then(|scope| scope.remaining.first().copied())
                    .ok_or_else(|| "missing current each-player draw seat".into()),
                mtg_contracts::kir::PlayerExpr::Is {
                    who: mtg_contracts::kir::Ref::Target { slot },
                } => self
                    .entry(frame.stack_id)?
                    .player_targets
                    .get(*slot as usize)
                    .copied()
                    .ok_or_else(|| "missing draw target".into()),
                mtg_contracts::kir::PlayerExpr::Is {
                    who: mtg_contracts::kir::Ref::EventPlayer { .. },
                } => self
                    .entry(frame.stack_id)?
                    .kind
                    .event_player()?
                    .ok_or_else(|| "absent draw event player".into()),
                mtg_contracts::kir::PlayerExpr::Is {
                    who: mtg_contracts::kir::Ref::Controller { .. },
                } => Ok(self.entry(frame.stack_id)?.controller),
                // CR 108.3, 608.2h: the physical card's owner stays the
                // same after the preceding instruction shuffled the object.
                mtg_contracts::kir::PlayerExpr::OwnerOf {
                    object: mtg_contracts::kir::Ref::Target { slot },
                } => {
                    let target = self
                        .bound_object_target(self.entry(frame.stack_id)?, *slot as usize)
                        .ok_or("missing target for owner draw")?;
                    Ok(self.state.rules.cards[&target.card].owner)
                }
                _ => Err("unsupported draw recipient".into()),
            },

            _ => Err("not draw instruction".into()),
        }
    }

    pub(crate) fn creature_or_planeswalker(&self, id: ObjectId) -> bool {
        self.current(id).is_ok_and(|o| o.present_on_battlefield())
            && self
                .characteristics(id)
                .is_ok_and(|c| c.has_type(CardKind::Creature))
    }
    pub(crate) fn object_target_legal(
        &self,
        entry: &StackInstance,
        slot: usize,
        object: ObjectId,
    ) -> bool {
        self.program_for(entry).is_ok_and(|program| {
            program
                .targets
                .get(if program.variable_target_count().is_some() {
                    0
                } else {
                    slot
                })
                .is_some_and(|spec| self.object_target_legal_for(entry, object, spec))
        })
    }

    pub(super) fn object_target_legal_for(
        &self,
        entry: &StackInstance,
        object: ObjectId,
        spec: &mtg_contracts::TargetSpec,
    ) -> bool {
        self.targeting_characteristics(entry).is_ok_and(|source| {
            self.object_target_legal_by(object, entry.controller, &source, Some(entry.source), spec)
        })
    }

    /// CR 115.1, 601.2c, 608.2b: whether `object` is a legal target of an
    /// object domain for `controller`, with `source` the targeting
    /// characteristics and `source_object` its identity, which a
    /// controlled-creature target and "another" exclude. Announcement before
    /// the stack entry exists and the resolution recheck share this test.
    pub(super) fn object_target_legal_by(
        &self,
        object: ObjectId,
        controller: PlayerId,
        source: &Characteristics,
        source_object: Option<ObjectId>,
        spec: &mtg_contracts::TargetSpec,
    ) -> bool {
        if self.protected_from(object, source) {
            return false;
        }
        match spec.domain {
            TargetDomain::BattlefieldPermanent | TargetDomain::ControlledBattlefieldPermanent => {
                self.permanent_target_legal(object, controller, source_object, spec)
            }
            TargetDomain::GraveyardCard { .. }
            | TargetDomain::OwnGraveyardCreatureCard
            | TargetDomain::OwnGraveyardCard => self
                .current(object)
                .is_ok_and(|o| self.graveyard_target_legal_for(o, controller, spec)),
            TargetDomain::CreatureOrPlaneswalker => {
                self.creature_or_planeswalker(object) && self.targetable_by(object, controller)
            }
            TargetDomain::ControlledBattlefieldCreature => {
                self.is_battlefield_creature(object)
                    && self.targetable_by(object, controller)
                    && self
                        .current(object)
                        .is_ok_and(|o| o.controller == controller && Some(o.id) != source_object)
            }
            TargetDomain::BlockingBattlefieldCreature => {
                self.is_blocking_creature(object) && self.targetable_by(object, controller)
            }
            _ => self.is_battlefield_creature(object) && self.targetable_by(object, controller),
        }
    }

    pub(super) fn execute_staples_instruction(
        &mut self,
        frame: Resolving,
        instruction: Instruction,
    ) -> Result<(), String> {
        match instruction {
            // CR 701.8: one announced creature, destroyed as a creature.
            _ if instruction.target_destroy().is_some() => {
                let Some((target_slot, _, _, source_block)) = instruction.target_destroy() else {
                    unreachable!()
                };
                let target = self.entry(frame.stack_id)?.targets[target_slot as usize];
                if frame.legal_targets.get(target_slot as usize) == Some(&true)
                    && self.is_battlefield_creature(target)
                {
                    return self.complete_graveyard_group(
                        self.instruction_cause(&frame, source_block)?,
                        EventGroup::Occurrence {
                            proposed: ProposedEvent::DestroyCreature { object: target },
                        },
                        GroupFinish::Instruction {
                            frame,
                            result: GroupResult::Effect,
                        },
                    );
                }
                self.finish_instruction(frame, InstructionOutcome::Effect { actual: None })
            }

            _ => Err("not a staples instruction".into()),
        }
    }

    pub(super) fn apply_search_shuffle_word(
        &mut self,
        player: PlayerId,
        index: u32,
        replacement_offset: Option<usize>,
    ) -> Result<EventOutcome, String> {
        let offset = if let Some(offset) = replacement_offset {
            offset
        } else {
            return Err("shuffle continuation invariant".into());
        };
        let rules = self.rules_mut();
        let library = rules
            .libraries
            .get_mut(&player)
            .ok_or("missing search library")?;
        if index == 0 || index as usize >= library.len().saturating_sub(offset) {
            return Err("shuffle range".into());
        }
        // A tucked commander can bring the physical library to all 100 owned
        // cards. The shared sampler uses that common bound, not setup's 99.
        // A countered copy or a tucked token may share a library with all
        // physical cards until the next SBA check (CR 704.5d/e).
        let limit = if replacement_offset.is_some() {
            capacities::OWNED_LIMIT + capacities::TOKEN_CAPACITY as usize
        } else {
            capacities::OWNED_LIMIT
        };
        if library.len() > limit {
            return Err("shuffle library exceeds the physical-card sampler range".into());
        }
        let state = rules
            .match_state
            .as_mut()
            .ok_or("missing trusted match randomness")?;
        let stream = setup::stream_id(
            setup::LIBRARY_SHUFFLE,
            player,
            state.library_shuffles[player.0 as usize],
        )?;
        let word = state.random.word(stream)?;
        let position = OpenStream {
            stream,
            word: state.random.open.unwrap().word - 1,
        };
        let selected = setup::sample_bounded_word(word, index + 1);
        if let Some(j) = selected {
            library.swap(offset + index as usize, offset + j as usize);
        }
        Ok(EventOutcome::LibraryShuffledWord {
            player,
            index,
            word,
            selected,
            position,
        })
    }

    pub(super) fn valid_staples_choice(
        &self,
        frame: &Resolving,
        choice: &ResolutionChoice,
        instruction: &Instruction,
    ) -> bool {
        let Ok(entry) = self.entry(frame.stack_id) else {
            return false;
        };
        match (choice, instruction) {
            (
                ResolutionChoice::OptionalEffect {
                    player,
                    target,
                    source_block,
                },
                i,
            ) if i.target_destroy().is_some_and(|(_, optional, ..)| optional) => {
                let (target_slot, _, _, b) = i.target_destroy().unwrap();
                *player == entry.controller
                    && *source_block == b
                    && entry.targets.get(target_slot as usize) == Some(target)
            }

            _ => false,
        }
    }
}
