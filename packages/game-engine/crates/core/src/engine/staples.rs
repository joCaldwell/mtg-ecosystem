//! Finite staples. Pinned CR 115.1, 400.7, 603.5, 603.10a, 608.2b,
//! 701.8a-b, 701.23a-b/e, 701.24a-b and 903.9a-b:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VisibleSearchLand {
    pub handle: ObjectHandle,
    pub oracle_id: String,
}
// Derived authority only; never serialized alongside a resolution choice.
struct LandSearch<'a> {
    typed: Option<&'a TypedLandSearch>,
    maximum: u32,
    split: bool,
    force_tapped: bool,
    source_block: u32,
}
impl<'a> LandSearch<'a> {
    fn instruction(instruction: &'a Instruction) -> Option<Self> {
        let (typed, maximum, split, force_tapped, source_block) = match instruction {
            _ => return None,
        };
        Some(Self {
            typed,
            maximum,
            split,
            force_tapped,
            source_block,
        })
    }
    fn accepts(&self, c: &Characteristics) -> bool {
        self.typed
            .map_or_else(|| c.kind == CardKind::Land && c.basic, |s| s.accepts(c))
    }
    fn force_tapped(&self) -> bool {
        self.force_tapped
    }
}
impl RulesTxn {
    fn land_search_for<'a>(&'a self, frame: &'a Resolving) -> Option<LandSearch<'a>> {
        LandSearch::instruction(
            self.program_for(self.entry(frame.stack_id).ok()?)
                .ok()?
                .instructions
                .get(frame.cursor as usize)?,
        )
    }

    fn active_land_search(&self) -> Option<LandSearch<'_>> {
        let Frame::ResolutionChoice { frame, choice } = &self.state.rules.frame else {
            return None;
        };
        {
            return None;
        }
        self.land_search_for(frame)
    }

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
                _ => Err("unsupported draw recipient".into()),
            },

            _ => Err("not draw instruction".into()),
        }
    }

    pub(crate) fn graveyard_target_legal(
        &self,
        object: &Object,
        player: PlayerId,
        domain: TargetDomain,
    ) -> bool {
        self.player_graveyard_card(object, player)
            && match domain {
                TargetDomain::OwnGraveyardCard => true,
                TargetDomain::OwnGraveyardCreatureCard => self
                    .definition(object.id.card)
                    .is_ok_and(|d| d.characteristics.kind == CardKind::Creature),
                _ => false,
            }
    }
    pub(crate) fn creature_or_planeswalker(&self, id: ObjectId) -> bool {
        self.current(id).is_ok_and(|o| o.present_on_battlefield())
            && self
                .characteristics(id)
                .is_ok_and(|c| c.has_type(CardKind::Creature))
    }
    pub(crate) fn object_target_legal(&self, entry: &StackInstance, object: ObjectId) -> bool {
        let announced = matches!(self.state.rules.frame, Frame::Targets { .. })
            && self.top().is_ok_and(|top| top.id == entry.id);
        let program = if announced {
            self.program()
        } else {
            self.program_for(entry)
        };
        let spec = program.ok().and_then(|p| p.targets.first());
        self.object_target_legal_for(entry, object, spec)
    }
    pub(super) fn object_target_legal_for(
        &self,
        entry: &StackInstance,
        object: ObjectId,
        spec: Option<&mtg_contracts::TargetSpec>,
    ) -> bool {
        if !self
            .targeting_characteristics(entry)
            .is_ok_and(|source| !self.protected_from(object, &source))
        {
            return false;
        }
        match spec.map(|t| t.domain) {
            Some(
                TargetDomain::BattlefieldPermanent | TargetDomain::ControlledBattlefieldPermanent,
            ) => spec.is_some_and(|t| self.permanent_target_legal(object, entry.controller, t)),
            Some(
                domain @ (TargetDomain::OwnGraveyardCreatureCard | TargetDomain::OwnGraveyardCard),
            ) => self
                .current(object)
                .is_ok_and(|o| self.graveyard_target_legal(o, entry.controller, domain)),
            Some(TargetDomain::CreatureOrPlaneswalker) => {
                self.creature_or_planeswalker(object)
                    && self.targetable_by(object, entry.controller)
            }
            Some(TargetDomain::ControlledBattlefieldCreature) => {
                self.is_battlefield_creature(object)
                    && self.targetable_by(object, entry.controller)
                    && self
                        .current(object)
                        .is_ok_and(|o| o.controller == entry.controller && o.id != entry.source)
            }
            Some(TargetDomain::BlockingBattlefieldCreature) => {
                self.is_blocking_creature(object) && self.targetable_by(object, entry.controller)
            }
            _ => {
                self.is_battlefield_creature(object) && self.targetable_by(object, entry.controller)
            }
        }
    }
    /// CR 701.23a/401.2: private search inspection is distinct from the
    /// selectable subset. No selectable handle or library position is created.
    pub(super) fn search_library_identities(
        &self,
        player: PlayerId,
    ) -> Result<Vec<String>, String> {
        let mut cards = self
            .state
            .rules
            .libraries
            .get(&player)
            .into_iter()
            .flatten()
            .map(|id| Ok(self.definition(*id)?.oracle_id.clone()))
            .collect::<Result<Vec<_>, String>>()?;
        cards.sort();
        Ok(cards)
    }
    pub(crate) fn search_lands(&self, player: PlayerId) -> Vec<ObjectId> {
        let descriptor = self.active_land_search();
        // CardId order is physical order, never library order (CR 401.2).
        let mut cards = self
            .state
            .rules
            .objects
            .values()
            .filter(|o| {
                o.zone == (Zone::Library { owner: player })
                    && self.definition(o.id.card).is_ok_and(|d| {
                        descriptor.as_ref().map_or_else(
                            || d.characteristics.kind == CardKind::Land && d.characteristics.basic,
                            |s| s.accepts(&d.characteristics),
                        )
                    })
            })
            .map(|o| o.id)
            .collect::<Vec<_>>();
        cards.sort_by_key(|o| (self.definition(o.card).unwrap().oracle_id.clone(), o.card));
        cards
    }
    pub(super) fn search_handle(
        &self,
        player: PlayerId,
        object: ObjectId,
    ) -> Result<ObjectHandle, String> {
        {
            return Err("no private land search".into());
        }
        let index = self
            .search_lands(player)
            .iter()
            .position(|o| *o == object)
            .ok_or("not a search candidate")?;
        let token = self.search_decision_token(player)?;
        Ok(ObjectHandle(format!("{}:search:{index}", token.0)))
    }
    pub(crate) fn resolve_search_handle(
        &self,
        player: PlayerId,
        handle: &ObjectHandle,
    ) -> Result<ObjectId, String> {
        let prefix = format!("{}:search:", self.search_decision_token(player)?.0);
        let index = handle
            .0
            .strip_prefix(&prefix)
            .and_then(|n| n.parse::<usize>().ok())
            .ok_or("stale or foreign search handle")?;
        let object = self
            .search_lands(player)
            .get(index)
            .copied()
            .ok_or("invalid search handle")?;
        if self.search_handle(player, object)? != *handle {
            return Err("noncanonical search handle".into());
        }
        Ok(object)
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

    pub(super) fn return_target_to_owner_hand(
        &mut self,
        frame: Resolving,
        target: ObjectId,
        block: u32,
    ) -> Result<(), String> {
        if self.current(target).is_err() {
            return self
                .finish_instruction(frame, InstructionOutcome::ObjectNoLongerPresent { target });
        }
        let owner = self.state.rules.cards[&target.card].owner;
        // CR903.9b offers the owner the hand-move replacement from ANY zone.
        // Declining leaves an ordinary hand card (still the physical Commander).
        if self.state.rules.commanders.history(target.card).is_some() {
            self.rules_mut().frame = Frame::CommanderReturn {
                object: target,
                destination: Zone::Hand { owner },
                continuation: CommanderContinuation::SpellMove {
                    frame,
                    source_block: block,
                },
            };
            return Ok(());
        }
        let movement = self.move_object_for(
            target,
            Zone::Hand { owner },
            self.instruction_cause(&frame, block)?,
        )?;
        self.finish_instruction(frame, InstructionOutcome::Moved { movement })
    }
    pub(super) fn validate_staples_response(
        &self,
        player: PlayerId,
        response: &Response,
    ) -> Option<Result<(), String>> {
        let Frame::ResolutionChoice { choice, .. } = &self.state.rules.frame else {
            return None;
        };
        match (choice, response) {
            _ => None,
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
        let random = &mut rules
            .match_state
            .as_mut()
            .ok_or("missing trusted match randomness")?
            .random;
        let before = random.word_position;
        let word = random.word()?;
        let selected = setup::sample_bounded_word(word, index + 1);
        if let Some(j) = selected {
            library.swap(offset + index as usize, offset + j as usize);
        }
        Ok(EventOutcome::LibraryShuffledWord {
            player,
            index,
            word,
            selected,
            before,
            after: random.word_position,
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
